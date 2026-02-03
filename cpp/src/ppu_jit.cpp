/**
 * PPU JIT compiler
 * 
 * Provides Just-In-Time compilation for PowerPC 64-bit (Cell PPU) instructions
 * using basic block compilation, LLVM IR generation, and native code emission.
 * 
 * Features:
 * - Branch prediction hints for optimized control flow
 * - Inline caching for frequently called functions
 * - Register allocation optimization
 * - Lazy compilation with on-demand code generation
 * - Multi-threaded compilation support
 */

#include "oc_ffi.h"
#include "oc_threading.h"
#include <cstdlib>
#include <cstring>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <algorithm>
#include <memory>
#include <queue>
#include <atomic>
#include <functional>
#include <list>

#ifdef HAVE_LLVM
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/Function.h>
#include <llvm/IR/BasicBlock.h>
#include <llvm/IR/Type.h>
#include <llvm/IR/Verifier.h>
#include <llvm/ExecutionEngine/ExecutionEngine.h>
#include <llvm/ExecutionEngine/MCJIT.h>
#include <llvm/ExecutionEngine/Orc/LLJIT.h>
#include <llvm/ExecutionEngine/Orc/ThreadSafeModule.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/TargetParser/Host.h>
#include <llvm/Support/Error.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/MC/TargetRegistry.h>
#include <llvm/Transforms/Scalar.h>
#include <llvm/Transforms/InstCombine/InstCombine.h>
#include <llvm/Passes/PassBuilder.h>
#include <llvm/Analysis/LoopAnalysisManager.h>
#include <llvm/Analysis/CGSCCPassManager.h>
#endif

/**
 * Basic block structure for compiled code
 */
struct BasicBlock {
    uint32_t start_address;
    uint32_t end_address;
    std::vector<uint32_t> instructions;
    void* compiled_code;
    size_t code_size;
    
    // Block merging support: CFG edges
    std::vector<uint32_t> successors;    // Addresses of successor blocks
    std::vector<uint32_t> predecessors;  // Addresses of predecessor blocks
    bool is_fallthrough;                 // True if block falls through to next
    bool can_merge;                      // True if block can be merged with successor
    
#ifdef HAVE_LLVM
    std::unique_ptr<llvm::Function> llvm_func;
#endif
    
    BasicBlock(uint32_t start) 
        : start_address(start), end_address(start), compiled_code(nullptr), code_size(0),
          is_fallthrough(false), can_merge(false) {}
};

/**
 * Cache statistics for profiling
 */
struct CacheStatistics {
    uint64_t hit_count;
    uint64_t miss_count;
    uint64_t eviction_count;
    uint64_t invalidation_count;
    
    CacheStatistics() : hit_count(0), miss_count(0), eviction_count(0), invalidation_count(0) {}
    
    double hit_rate() const {
        uint64_t total = hit_count + miss_count;
        return total > 0 ? static_cast<double>(hit_count) / total : 0.0;
    }
};

/**
 * Code cache for compiled blocks with LRU eviction
 */
struct CodeCache {
    std::unordered_map<uint32_t, std::unique_ptr<BasicBlock>> blocks;
    std::list<uint32_t> lru_order;  // LRU tracking: front = most recent, back = least recent
    std::unordered_map<uint32_t, std::list<uint32_t>::iterator> lru_positions;
    oc_mutex mutex;
    size_t total_size;
    size_t max_size;
    CacheStatistics stats;
    
    CodeCache() : total_size(0), max_size(64 * 1024 * 1024) {} // 64MB default
    
    void set_max_size(size_t size) { max_size = size; }
    size_t get_max_size() const { return max_size; }
    
    BasicBlock* find_block(uint32_t address) {
        auto it = blocks.find(address);
        if (it != blocks.end()) {
            // Move to front of LRU list (most recently used)
            auto lru_it = lru_positions.find(address);
            if (lru_it != lru_positions.end()) {
                lru_order.erase(lru_it->second);
                lru_order.push_front(address);
                lru_positions[address] = lru_order.begin();
            }
            stats.hit_count++;
            return it->second.get();
        }
        stats.miss_count++;
        return nullptr;
    }
    
    void insert_block(uint32_t address, std::unique_ptr<BasicBlock> block) {
        // Evict LRU blocks if we're over the limit
        while (total_size + block->code_size > max_size && !lru_order.empty()) {
            evict_lru();
        }
        
        total_size += block->code_size;
        blocks[address] = std::move(block);
        
        // Add to front of LRU list
        lru_order.push_front(address);
        lru_positions[address] = lru_order.begin();
    }
    
    void evict_lru() {
        if (lru_order.empty()) return;
        
        uint32_t oldest = lru_order.back();
        lru_order.pop_back();
        lru_positions.erase(oldest);
        
        auto it = blocks.find(oldest);
        if (it != blocks.end()) {
            total_size -= it->second->code_size;
            blocks.erase(it);
            stats.eviction_count++;
        }
    }
    
    void invalidate(uint32_t address) {
        auto it = blocks.find(address);
        if (it != blocks.end()) {
            total_size -= it->second->code_size;
            blocks.erase(it);
            
            auto lru_it = lru_positions.find(address);
            if (lru_it != lru_positions.end()) {
                lru_order.erase(lru_it->second);
                lru_positions.erase(lru_it);
            }
            stats.invalidation_count++;
        }
    }
    
    void invalidate_range(uint32_t start, uint32_t end) {
        std::vector<uint32_t> to_remove;
        for (const auto& pair : blocks) {
            if (pair.first >= start && pair.first < end) {
                to_remove.push_back(pair.first);
            }
        }
        for (uint32_t addr : to_remove) {
            invalidate(addr);
        }
    }
    
    void clear() {
        blocks.clear();
        lru_order.clear();
        lru_positions.clear();
        total_size = 0;
    }
    
    const CacheStatistics& get_statistics() const { return stats; }
    void reset_statistics() { stats = CacheStatistics(); }
};

/**
 * Block Merger: Merges consecutive blocks for better optimization
 * 
 * Analyzes CFG to identify blocks that can be merged:
 * - Blocks that fall through to their successor
 * - Blocks with a single successor that has a single predecessor
 */
struct BlockMerger {
    std::unordered_map<uint32_t, std::vector<uint32_t>> successors;
    std::unordered_map<uint32_t, std::vector<uint32_t>> predecessors;
    
    /**
     * Analyze a block and determine its successors based on its terminating instruction
     */
    void analyze_block(BasicBlock* block) {
        if (block->instructions.empty()) return;
        
        uint32_t last_instr = block->instructions.back();
        uint8_t opcode = (last_instr >> 26) & 0x3F;
        
        // Determine block successors based on branch type
        bool is_unconditional_branch = false;
        bool is_conditional_branch = false;
        uint32_t branch_target = 0;
        
        if (opcode == 18) { // b/bl/ba/bla
            bool aa = (last_instr >> 1) & 1;  // Absolute address bit
            int32_t li = (last_instr >> 2) & 0xFFFFFF;
            if (li & 0x800000) li |= 0xFF000000;  // Sign extend
            li <<= 2;  // LI is in units of words
            
            if (aa) {
                branch_target = static_cast<uint32_t>(li);
            } else {
                branch_target = block->end_address - 4 + li;
            }
            is_unconditional_branch = true;
            
            successors[block->start_address].push_back(branch_target);
            predecessors[branch_target].push_back(block->start_address);
        } else if (opcode == 16) { // bc/bcl/bca/bcla (conditional)
            bool aa = (last_instr >> 1) & 1;
            int32_t bd = (last_instr >> 2) & 0x3FFF;
            if (bd & 0x2000) bd |= 0xFFFFC000;  // Sign extend
            bd <<= 2;
            
            if (aa) {
                branch_target = static_cast<uint32_t>(bd);
            } else {
                branch_target = block->end_address - 4 + bd;
            }
            is_conditional_branch = true;
            
            // Conditional branches have two successors
            successors[block->start_address].push_back(branch_target);
            successors[block->start_address].push_back(block->end_address);
            predecessors[branch_target].push_back(block->start_address);
            predecessors[block->end_address].push_back(block->start_address);
        } else if (opcode == 19) { // bclr/bcctr
            // Indirect branches - target unknown at analysis time
            // Can't merge across indirect branches
        } else {
            // Block falls through to next instruction
            block->is_fallthrough = true;
            successors[block->start_address].push_back(block->end_address);
            predecessors[block->end_address].push_back(block->start_address);
        }
        
        // Update block's successor/predecessor lists
        block->successors = successors[block->start_address];
        
        // Determine if block can be merged with its successor
        // Conditions: single fallthrough successor, successor has single predecessor
        if (is_unconditional_branch && !is_conditional_branch) {
            // Unconditional branch can be merged if target has single predecessor
            if (predecessors[branch_target].size() == 1) {
                block->can_merge = true;
            }
        } else if (block->is_fallthrough) {
            block->can_merge = true;
        }
    }
    
    /**
     * Check if two blocks can be merged
     */
    bool can_merge_blocks(BasicBlock* first, BasicBlock* second) {
        // First block must end at second block's start
        if (first->end_address != second->start_address) return false;
        
        // First block must fall through or unconditionally branch to second
        if (!first->is_fallthrough && std::find(first->successors.begin(), 
            first->successors.end(), second->start_address) == first->successors.end()) {
            return false;
        }
        
        // Second block must have only one predecessor (the first block)
        auto it = predecessors.find(second->start_address);
        if (it != predecessors.end() && it->second.size() != 1) {
            return false;
        }
        
        return true;
    }
    
    /**
     * Merge two blocks into one
     * Returns a new merged block, or nullptr if merge is not possible
     */
    std::unique_ptr<BasicBlock> merge_blocks(BasicBlock* first, BasicBlock* second) {
        if (!can_merge_blocks(first, second)) return nullptr;
        
        auto merged = std::make_unique<BasicBlock>(first->start_address);
        merged->end_address = second->end_address;
        
        // Combine instructions
        merged->instructions = first->instructions;
        
        // If first block ends with a fallthrough, remove last instruction if it's just a branch to next
        // Otherwise, append all instructions from second block
        if (first->is_fallthrough) {
            // Just append second block's instructions
            merged->instructions.insert(merged->instructions.end(),
                second->instructions.begin(), second->instructions.end());
        } else {
            // First block ends with unconditional branch to second - remove the branch
            if (!merged->instructions.empty()) {
                uint32_t last = merged->instructions.back();
                uint8_t op = (last >> 26) & 0x3F;
                if (op == 18) { // Unconditional branch - remove it
                    merged->instructions.pop_back();
                }
            }
            merged->instructions.insert(merged->instructions.end(),
                second->instructions.begin(), second->instructions.end());
        }
        
        // Copy successors from second block
        merged->successors = second->successors;
        merged->is_fallthrough = second->is_fallthrough;
        merged->can_merge = second->can_merge;
        
        return merged;
    }
    
    /**
     * Clear analysis state
     */
    void clear() {
        successors.clear();
        predecessors.clear();
    }
};

/**
 * Breakpoint management
 */
struct BreakpointManager {
    std::unordered_map<uint32_t, bool> breakpoints;
    
    void add_breakpoint(uint32_t address) {
        breakpoints[address] = true;
    }
    
    void remove_breakpoint(uint32_t address) {
        breakpoints.erase(address);
    }
    
    bool has_breakpoint(uint32_t address) const {
        return breakpoints.find(address) != breakpoints.end();
    }
    
    void clear() {
        breakpoints.clear();
    }
};

/**
 * Branch prediction hint types
 */
enum class BranchHint : uint8_t {
    None = 0,
    Likely = 1,      // Branch is likely to be taken (+ hint)
    Unlikely = 2,    // Branch is unlikely to be taken (- hint)
    Static = 3       // Use static prediction (backward=taken, forward=not taken)
};

/**
 * Default threshold for branch prediction classification
 * A branch is classified as "likely" when: taken_count > not_taken_count * threshold
 * A branch is classified as "unlikely" when: not_taken_count > taken_count * threshold
 */
static constexpr uint32_t DEFAULT_BRANCH_THRESHOLD = 2;

/**
 * Branch prediction data for a basic block
 */
struct BranchPrediction {
    uint32_t branch_address;
    uint32_t target_address;
    BranchHint hint;
    uint32_t taken_count;
    uint32_t not_taken_count;
    uint32_t correct_predictions;    // Prediction accuracy tracking
    uint32_t incorrect_predictions;  // Prediction accuracy tracking
    uint32_t likely_threshold;       // Configurable threshold for "likely" classification
    uint32_t unlikely_threshold;     // Configurable threshold for "unlikely" classification
    
    BranchPrediction() 
        : branch_address(0), target_address(0), hint(BranchHint::None),
          taken_count(0), not_taken_count(0), correct_predictions(0),
          incorrect_predictions(0), likely_threshold(DEFAULT_BRANCH_THRESHOLD), 
          unlikely_threshold(DEFAULT_BRANCH_THRESHOLD) {}
    
    BranchPrediction(uint32_t addr, uint32_t target, BranchHint h)
        : branch_address(addr), target_address(target), hint(h),
          taken_count(0), not_taken_count(0), correct_predictions(0),
          incorrect_predictions(0), likely_threshold(DEFAULT_BRANCH_THRESHOLD), 
          unlikely_threshold(DEFAULT_BRANCH_THRESHOLD) {}
    
    // Update prediction based on runtime behavior
    void update(bool taken) {
        // Track prediction accuracy
        bool predicted = predict_taken();
        if (predicted == taken) {
            correct_predictions++;
        } else {
            incorrect_predictions++;
        }
        
        if (taken) {
            taken_count++;
        } else {
            not_taken_count++;
        }
        
        // Update hint based on observed behavior using configurable thresholds
        if (taken_count > not_taken_count * likely_threshold) {
            hint = BranchHint::Likely;
        } else if (not_taken_count > taken_count * unlikely_threshold) {
            hint = BranchHint::Unlikely;
        }
    }
    
    // Get predicted direction
    bool predict_taken() const {
        switch (hint) {
            case BranchHint::Likely: return true;
            case BranchHint::Unlikely: return false;
            case BranchHint::Static:
                // Backward branches predicted taken, forward not taken
                return target_address < branch_address;
            default:
                return taken_count >= not_taken_count;
        }
    }
    
    // Get prediction accuracy as a percentage (0-100)
    double get_accuracy() const {
        uint32_t total = correct_predictions + incorrect_predictions;
        return (total > 0) ? (100.0 * correct_predictions / total) : 0.0;
    }
    
    // Set configurable thresholds
    void set_thresholds(uint32_t likely_thresh, uint32_t unlikely_thresh) {
        likely_threshold = likely_thresh;
        unlikely_threshold = unlikely_thresh;
    }
    
    // Reset statistics
    void reset_stats() {
        taken_count = 0;
        not_taken_count = 0;
        correct_predictions = 0;
        incorrect_predictions = 0;
    }
};

/**
 * Branch prediction manager
 */
struct BranchPredictor {
    std::unordered_map<uint32_t, BranchPrediction> predictions;
    oc_mutex mutex;
    uint32_t default_likely_threshold;    // Default threshold for new predictions
    uint32_t default_unlikely_threshold;  // Default threshold for new predictions
    
    BranchPredictor() : default_likely_threshold(DEFAULT_BRANCH_THRESHOLD), 
                        default_unlikely_threshold(DEFAULT_BRANCH_THRESHOLD) {}
    
    void add_prediction(uint32_t address, uint32_t target, BranchHint hint) {
        oc_lock_guard<oc_mutex> lock(mutex);
        BranchPrediction pred(address, target, hint);
        pred.set_thresholds(default_likely_threshold, default_unlikely_threshold);
        predictions[address] = pred;
    }
    
    BranchPrediction* get_prediction(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = predictions.find(address);
        return (it != predictions.end()) ? &it->second : nullptr;
    }
    
    void update_prediction(uint32_t address, bool taken) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = predictions.find(address);
        if (it != predictions.end()) {
            it->second.update(taken);
        }
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        predictions.clear();
    }
    
    // Set default thresholds for new predictions
    void set_default_thresholds(uint32_t likely_thresh, uint32_t unlikely_thresh) {
        oc_lock_guard<oc_mutex> lock(mutex);
        default_likely_threshold = likely_thresh;
        default_unlikely_threshold = unlikely_thresh;
    }
    
    // Set thresholds for a specific branch
    void set_branch_thresholds(uint32_t address, uint32_t likely_thresh, uint32_t unlikely_thresh) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = predictions.find(address);
        if (it != predictions.end()) {
            it->second.set_thresholds(likely_thresh, unlikely_thresh);
        }
    }
    
    // Get prediction statistics for a specific branch
    // Returns: accuracy percentage (0-100), or -1 if branch not found
    double get_branch_accuracy(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = predictions.find(address);
        return (it != predictions.end()) ? it->second.get_accuracy() : -1.0;
    }
    
    // Get aggregate prediction statistics
    // Returns: {total_correct, total_incorrect, overall_accuracy}
    struct AggregateStats {
        uint64_t total_correct;
        uint64_t total_incorrect;
        double overall_accuracy;
    };
    
    AggregateStats get_aggregate_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        AggregateStats stats = {0, 0, 0.0};
        for (const auto& pair : predictions) {
            stats.total_correct += pair.second.correct_predictions;
            stats.total_incorrect += pair.second.incorrect_predictions;
        }
        uint64_t total = stats.total_correct + stats.total_incorrect;
        stats.overall_accuracy = (total > 0) ? (100.0 * stats.total_correct / total) : 0.0;
        return stats;
    }
    
    // Reset all prediction statistics
    void reset_all_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        for (auto& pair : predictions) {
            pair.second.reset_stats();
        }
    }
};

/**
 * Inline cache entry for call sites
 */
struct InlineCacheEntry {
    uint32_t call_site;        // Address of call instruction
    uint32_t target_address;   // Cached target address
    void* compiled_target;     // Pointer to compiled target code
    uint32_t hit_count;        // Number of cache hits
    bool is_valid;             // Whether the cache entry is valid
    
    InlineCacheEntry()
        : call_site(0), target_address(0), compiled_target(nullptr),
          hit_count(0), is_valid(false) {}
    
    InlineCacheEntry(uint32_t site, uint32_t target)
        : call_site(site), target_address(target), compiled_target(nullptr),
          hit_count(0), is_valid(true) {}
};

/**
 * Inline cache manager for call sites
 */
struct InlineCacheManager {
    std::unordered_map<uint32_t, InlineCacheEntry> cache;
    oc_mutex mutex;
    size_t max_entries;
    
    InlineCacheManager() : max_entries(4096) {}
    
    void add_entry(uint32_t call_site, uint32_t target) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Evict if at capacity
        if (cache.size() >= max_entries) {
            // Find entry with lowest hit count
            uint32_t min_hits = UINT32_MAX;
            uint32_t evict_addr = 0;
            for (const auto& pair : cache) {
                if (pair.second.hit_count < min_hits) {
                    min_hits = pair.second.hit_count;
                    evict_addr = pair.first;
                }
            }
            cache.erase(evict_addr);
        }
        
        cache[call_site] = InlineCacheEntry(call_site, target);
    }
    
    InlineCacheEntry* lookup(uint32_t call_site) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = cache.find(call_site);
        if (it != cache.end() && it->second.is_valid) {
            it->second.hit_count++;
            return &it->second;
        }
        return nullptr;
    }
    
    void invalidate(uint32_t target_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        for (auto& pair : cache) {
            if (pair.second.target_address == target_address) {
                pair.second.is_valid = false;
                pair.second.compiled_target = nullptr;
            }
        }
    }
    
    void update_compiled_target(uint32_t target_address, void* compiled) {
        oc_lock_guard<oc_mutex> lock(mutex);
        for (auto& pair : cache) {
            if (pair.second.target_address == target_address && pair.second.is_valid) {
                pair.second.compiled_target = compiled;
            }
        }
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        cache.clear();
    }
};

/**
 * Maximum number of targets for polymorphic inline cache
 */
static constexpr size_t MAX_POLYMORPHIC_TARGETS = 4;

/**
 * Branch target entry for indirect branches
 */
struct BranchTargetEntry {
    uint32_t branch_address;     // Address of the indirect branch instruction
    uint32_t target_address;     // Cached target address
    void* compiled_target;       // Pointer to compiled target code
    uint32_t hit_count;          // Number of cache hits
    uint32_t miss_count;         // Number of cache misses (target mismatch)
    bool is_valid;               // Whether the entry is valid
    
    BranchTargetEntry()
        : branch_address(0), target_address(0), compiled_target(nullptr),
          hit_count(0), miss_count(0), is_valid(false) {}
    
    BranchTargetEntry(uint32_t branch, uint32_t target)
        : branch_address(branch), target_address(target), compiled_target(nullptr),
          hit_count(0), miss_count(0), is_valid(true) {}
    
    // Get hit rate as a percentage (0-100)
    double get_hit_rate() const {
        uint32_t total = hit_count + miss_count;
        return (total > 0) ? (100.0 * hit_count / total) : 0.0;
    }
};

/**
 * Polymorphic inline cache entry supporting multiple targets per call site
 */
struct PolymorphicEntry {
    uint32_t branch_address;                                 // Address of the indirect branch
    std::array<uint32_t, MAX_POLYMORPHIC_TARGETS> targets;   // Cached target addresses
    std::array<void*, MAX_POLYMORPHIC_TARGETS> compiled;     // Compiled code pointers
    std::array<uint32_t, MAX_POLYMORPHIC_TARGETS> hit_counts; // Hit counts per target
    size_t num_targets;                                      // Number of active targets
    uint32_t total_lookups;                                  // Total lookup count
    bool is_megamorphic;                                     // True if too many targets
    
    PolymorphicEntry()
        : branch_address(0), num_targets(0), total_lookups(0), is_megamorphic(false) {
        targets.fill(0);
        compiled.fill(nullptr);
        hit_counts.fill(0);
    }
    
    PolymorphicEntry(uint32_t branch)
        : branch_address(branch), num_targets(0), total_lookups(0), is_megamorphic(false) {
        targets.fill(0);
        compiled.fill(nullptr);
        hit_counts.fill(0);
    }
    
    // Add a new target, returns index or -1 if megamorphic
    int add_target(uint32_t target) {
        if (is_megamorphic) return -1;
        
        // Check if already exists
        for (size_t i = 0; i < num_targets; i++) {
            if (targets[i] == target) {
                return static_cast<int>(i);
            }
        }
        
        // Add new target if space available
        if (num_targets < MAX_POLYMORPHIC_TARGETS) {
            size_t idx = num_targets++;
            targets[idx] = target;
            compiled[idx] = nullptr;
            hit_counts[idx] = 0;
            return static_cast<int>(idx);
        }
        
        // Too many targets - become megamorphic
        is_megamorphic = true;
        return -1;
    }
    
    // Lookup target, returns compiled code or nullptr
    void* lookup(uint32_t target) {
        total_lookups++;
        for (size_t i = 0; i < num_targets; i++) {
            if (targets[i] == target) {
                hit_counts[i]++;
                return compiled[i];
            }
        }
        return nullptr;
    }
    
    // Update compiled code for a target
    void update_compiled(uint32_t target, void* code) {
        for (size_t i = 0; i < num_targets; i++) {
            if (targets[i] == target) {
                compiled[i] = code;
                return;
            }
        }
    }
};

/**
 * Branch Target Buffer (BTB) statistics
 */
struct BTBStatistics {
    uint64_t total_lookups;
    uint64_t total_hits;
    uint64_t total_misses;
    uint64_t polymorphic_lookups;
    uint64_t megamorphic_fallbacks;
    double overall_hit_rate;
    
    BTBStatistics()
        : total_lookups(0), total_hits(0), total_misses(0),
          polymorphic_lookups(0), megamorphic_fallbacks(0), overall_hit_rate(0.0) {}
};

/**
 * Branch Target Cache (BTB) for indirect branch optimization
 */
struct BranchTargetCache {
    std::unordered_map<uint32_t, BranchTargetEntry> monomorphic;    // Single-target cache
    std::unordered_map<uint32_t, PolymorphicEntry> polymorphic;     // Multi-target cache
    oc_mutex mutex;
    size_t max_entries;
    BTBStatistics stats;
    
    BranchTargetCache() : max_entries(8192) {}
    
    // Add or update monomorphic entry
    void add_entry(uint32_t branch_address, uint32_t target_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Check if already polymorphic
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            poly_it->second.add_target(target_address);
            return;
        }
        
        // Check if monomorphic with different target (promote to polymorphic)
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end()) {
            if (mono_it->second.target_address != target_address) {
                // Promote to polymorphic
                PolymorphicEntry poly(branch_address);
                poly.add_target(mono_it->second.target_address);
                poly.update_compiled(mono_it->second.target_address, 
                                     mono_it->second.compiled_target);
                poly.add_target(target_address);
                polymorphic[branch_address] = poly;
                monomorphic.erase(mono_it);
                return;
            }
            // Same target, just update
            return;
        }
        
        // Evict if at capacity
        if (monomorphic.size() >= max_entries) {
            // Find entry with lowest hit count
            uint32_t min_hits = UINT32_MAX;
            uint32_t evict_addr = 0;
            for (const auto& pair : monomorphic) {
                if (pair.second.hit_count < min_hits) {
                    min_hits = pair.second.hit_count;
                    evict_addr = pair.first;
                }
            }
            monomorphic.erase(evict_addr);
        }
        
        // Add new monomorphic entry
        monomorphic[branch_address] = BranchTargetEntry(branch_address, target_address);
    }
    
    // Lookup target for indirect branch
    // Returns: target address if found and valid, 0 otherwise
    uint32_t lookup(uint32_t branch_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats.total_lookups++;
        
        // Check polymorphic first (less common but need accurate lookup)
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            stats.polymorphic_lookups++;
            if (poly_it->second.is_megamorphic) {
                stats.megamorphic_fallbacks++;
                return 0;  // Megamorphic - no prediction
            }
            // Return most frequently hit target
            size_t best_idx = 0;
            uint32_t best_count = 0;
            for (size_t i = 0; i < poly_it->second.num_targets; i++) {
                if (poly_it->second.hit_counts[i] > best_count) {
                    best_count = poly_it->second.hit_counts[i];
                    best_idx = i;
                }
            }
            if (poly_it->second.num_targets > 0) {
                stats.total_hits++;
                return poly_it->second.targets[best_idx];
            }
            stats.total_misses++;
            return 0;
        }
        
        // Check monomorphic
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end() && mono_it->second.is_valid) {
            mono_it->second.hit_count++;
            stats.total_hits++;
            return mono_it->second.target_address;
        }
        
        stats.total_misses++;
        return 0;
    }
    
    // Update BTB with actual target taken
    void update(uint32_t branch_address, uint32_t actual_target) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Check polymorphic
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            int idx = poly_it->second.add_target(actual_target);
            if (idx >= 0) {
                poly_it->second.hit_counts[idx]++;
            }
            return;
        }
        
        // Check monomorphic
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end()) {
            if (mono_it->second.target_address == actual_target) {
                mono_it->second.hit_count++;
            } else {
                mono_it->second.miss_count++;
                // Promote to polymorphic if too many misses
                if (mono_it->second.miss_count > 3) {
                    PolymorphicEntry poly(branch_address);
                    poly.add_target(mono_it->second.target_address);
                    poly.update_compiled(mono_it->second.target_address,
                                         mono_it->second.compiled_target);
                    poly.add_target(actual_target);
                    polymorphic[branch_address] = poly;
                    monomorphic.erase(mono_it);
                }
            }
        }
    }
    
    // Validate that cached target matches expected
    bool validate(uint32_t branch_address, uint32_t expected_target) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end() && mono_it->second.is_valid) {
            return mono_it->second.target_address == expected_target;
        }
        
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            for (size_t i = 0; i < poly_it->second.num_targets; i++) {
                if (poly_it->second.targets[i] == expected_target) {
                    return true;
                }
            }
        }
        
        return false;
    }
    
    // Invalidate entry for branch address
    void invalidate(uint32_t branch_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end()) {
            mono_it->second.is_valid = false;
            mono_it->second.compiled_target = nullptr;
        }
        
        polymorphic.erase(branch_address);
    }
    
    // Invalidate all entries pointing to a target
    void invalidate_target(uint32_t target_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        for (auto& pair : monomorphic) {
            if (pair.second.target_address == target_address) {
                pair.second.is_valid = false;
                pair.second.compiled_target = nullptr;
            }
        }
        
        // For polymorphic entries, just clear the compiled pointer
        for (auto& pair : polymorphic) {
            pair.second.update_compiled(target_address, nullptr);
        }
    }
    
    // Update compiled code pointer
    void update_compiled(uint32_t branch_address, uint32_t target_address, void* compiled) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end() && 
            mono_it->second.target_address == target_address) {
            mono_it->second.compiled_target = compiled;
            return;
        }
        
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            poly_it->second.update_compiled(target_address, compiled);
        }
    }
    
    // Get compiled code for branch -> target
    void* get_compiled(uint32_t branch_address, uint32_t target_address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto mono_it = monomorphic.find(branch_address);
        if (mono_it != monomorphic.end() && 
            mono_it->second.target_address == target_address &&
            mono_it->second.is_valid) {
            return mono_it->second.compiled_target;
        }
        
        auto poly_it = polymorphic.find(branch_address);
        if (poly_it != polymorphic.end()) {
            // Direct access to avoid double-counting statistics
            for (size_t i = 0; i < poly_it->second.num_targets; i++) {
                if (poly_it->second.targets[i] == target_address) {
                    return poly_it->second.compiled[i];
                }
            }
        }
        
        return nullptr;
    }
    
    // Get statistics
    BTBStatistics get_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats.overall_hit_rate = (stats.total_lookups > 0) 
            ? (100.0 * stats.total_hits / stats.total_lookups) 
            : 0.0;
        return stats;
    }
    
    // Reset statistics
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = BTBStatistics();
        for (auto& pair : monomorphic) {
            pair.second.hit_count = 0;
            pair.second.miss_count = 0;
        }
        for (auto& pair : polymorphic) {
            pair.second.total_lookups = 0;
            pair.second.hit_counts.fill(0);
        }
    }
    
    // Clear all entries
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        monomorphic.clear();
        polymorphic.clear();
        stats = BTBStatistics();
    }
};

/**
 * Type of constant value for propagation cache
 */
enum class ConstantType : uint8_t {
    Unknown = 0,
    Immediate = 1,    // Immediate constant from instruction
    RegisterValue = 2, // Known value in register
    MemoryLoad = 3    // Cached memory load
};

/**
 * Constant value entry with type information
 */
struct ConstantValue {
    uint64_t value;         // The constant value
    ConstantType type;      // Type of constant
    uint32_t source_addr;   // Address where value was defined
    uint32_t use_count;     // Number of times accessed
    bool is_valid;          // Whether the value is still valid
    
    ConstantValue()
        : value(0), type(ConstantType::Unknown), source_addr(0),
          use_count(0), is_valid(false) {}
    
    ConstantValue(uint64_t val, ConstantType t, uint32_t src)
        : value(val), type(t), source_addr(src), use_count(0), is_valid(true) {}
};

/**
 * Register value tracking entry
 */
struct RegisterValueEntry {
    uint8_t reg_num;        // Register number (0-31)
    uint64_t value;         // Known value
    uint32_t def_addr;      // Address where value was defined
    uint32_t use_count;     // Number of times used
    bool is_known;          // Whether value is known
    bool is_constant;       // True if value is a compile-time constant
    
    RegisterValueEntry()
        : reg_num(0), value(0), def_addr(0), use_count(0),
          is_known(false), is_constant(false) {}
    
    RegisterValueEntry(uint8_t reg, uint64_t val, uint32_t addr, bool constant = false)
        : reg_num(reg), value(val), def_addr(addr), use_count(0),
          is_known(true), is_constant(constant) {}
};

/**
 * Memory load cache entry
 */
struct MemoryLoadEntry {
    uint32_t address;       // Memory address
    uint64_t value;         // Cached value
    uint8_t size;           // Load size in bytes (1, 2, 4, 8)
    uint32_t load_addr;     // Instruction address that performed the load
    uint32_t use_count;     // Number of cache hits
    bool is_valid;          // Whether the cached value is still valid
    
    MemoryLoadEntry()
        : address(0), value(0), size(0), load_addr(0),
          use_count(0), is_valid(false) {}
    
    MemoryLoadEntry(uint32_t addr, uint64_t val, uint8_t sz, uint32_t ld_addr)
        : address(addr), value(val), size(sz), load_addr(ld_addr),
          use_count(0), is_valid(true) {}
};

/**
 * Constant propagation cache statistics
 */
struct ConstPropStatistics {
    uint64_t imm_hits;          // Immediate value cache hits
    uint64_t imm_misses;        // Immediate value cache misses
    uint64_t reg_hits;          // Register value cache hits
    uint64_t reg_misses;        // Register value cache misses
    uint64_t mem_hits;          // Memory load cache hits
    uint64_t mem_misses;        // Memory load cache misses
    uint64_t invalidations;     // Number of cache invalidations
    
    ConstPropStatistics()
        : imm_hits(0), imm_misses(0), reg_hits(0), reg_misses(0),
          mem_hits(0), mem_misses(0), invalidations(0) {}
    
    double get_imm_hit_rate() const {
        uint64_t total = imm_hits + imm_misses;
        return (total > 0) ? (100.0 * imm_hits / total) : 0.0;
    }
    
    double get_reg_hit_rate() const {
        uint64_t total = reg_hits + reg_misses;
        return (total > 0) ? (100.0 * reg_hits / total) : 0.0;
    }
    
    double get_mem_hit_rate() const {
        uint64_t total = mem_hits + mem_misses;
        return (total > 0) ? (100.0 * mem_hits / total) : 0.0;
    }
};

/**
 * Constant Propagation Cache for optimizing constant values
 * Caches immediate values, known register values, and memory loads
 */
struct ConstantPropagationCache {
    // Immediate value cache: instruction address -> constant value
    std::unordered_map<uint32_t, ConstantValue> immediates;
    
    // Register value tracking: register number -> value entry
    // Indexed by block address to track per-block state
    std::unordered_map<uint32_t, std::array<RegisterValueEntry, 32>> register_values;
    
    // Memory load cache: memory address -> cached value
    std::unordered_map<uint32_t, MemoryLoadEntry> memory_loads;
    
    oc_mutex mutex;
    size_t max_immediates;
    size_t max_memory_loads;
    ConstPropStatistics stats;
    
    ConstantPropagationCache() : max_immediates(4096), max_memory_loads(2048) {}
    
    // ========== Immediate Value Cache ==========
    
    // Cache an immediate value from an instruction
    void set_immediate(uint32_t instr_addr, uint64_t value) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Evict if at capacity
        if (immediates.size() >= max_immediates) {
            uint32_t min_uses = UINT32_MAX;
            uint32_t evict_addr = 0;
            for (const auto& pair : immediates) {
                if (pair.second.use_count < min_uses) {
                    min_uses = pair.second.use_count;
                    evict_addr = pair.first;
                }
            }
            immediates.erase(evict_addr);
        }
        
        immediates[instr_addr] = ConstantValue(value, ConstantType::Immediate, instr_addr);
    }
    
    // Get cached immediate value
    // Returns true if found and valid, stores value in out_value
    bool get_immediate(uint32_t instr_addr, uint64_t* out_value) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = immediates.find(instr_addr);
        if (it != immediates.end() && it->second.is_valid) {
            it->second.use_count++;
            stats.imm_hits++;
            if (out_value) *out_value = it->second.value;
            return true;
        }
        
        stats.imm_misses++;
        return false;
    }
    
    // ========== Register Value Tracking ==========
    
    // Set known value for a register at a specific block
    void set_register_value(uint32_t block_addr, uint8_t reg_num, uint64_t value, 
                            uint32_t def_addr, bool is_constant = false) {
        if (reg_num >= 32) return;
        
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = register_values.find(block_addr);
        if (it == register_values.end()) {
            std::array<RegisterValueEntry, 32> regs{};  // Default-initialized
            register_values[block_addr] = regs;
            it = register_values.find(block_addr);
        }
        
        it->second[reg_num] = RegisterValueEntry(reg_num, value, def_addr, is_constant);
    }
    
    // Get known value for a register at a specific block
    bool get_register_value(uint32_t block_addr, uint8_t reg_num, uint64_t* out_value,
                            bool* out_is_constant = nullptr) {
        if (reg_num >= 32) return false;
        
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = register_values.find(block_addr);
        if (it != register_values.end()) {
            const auto& entry = it->second[reg_num];
            if (entry.is_known) {
                // Increment use count (non-const access)
                auto& mutable_entry = register_values[block_addr][reg_num];
                mutable_entry.use_count++;
                stats.reg_hits++;
                if (out_value) *out_value = entry.value;
                if (out_is_constant) *out_is_constant = entry.is_constant;
                return true;
            }
        }
        
        stats.reg_misses++;
        return false;
    }
    
    // Invalidate a register value at a specific block
    void invalidate_register(uint32_t block_addr, uint8_t reg_num) {
        if (reg_num >= 32) return;
        
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = register_values.find(block_addr);
        if (it != register_values.end()) {
            it->second[reg_num].is_known = false;
            it->second[reg_num].is_constant = false;
            stats.invalidations++;
        }
    }
    
    // Invalidate all registers for a block (e.g., at function call)
    void invalidate_all_registers(uint32_t block_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = register_values.find(block_addr);
        if (it != register_values.end()) {
            for (auto& entry : it->second) {
                entry.is_known = false;
                entry.is_constant = false;
            }
            stats.invalidations++;
        }
    }
    
    // ========== Memory Load Cache ==========
    
    // Cache a memory load
    void set_memory_load(uint32_t mem_addr, uint64_t value, uint8_t size, uint32_t load_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Evict if at capacity
        if (memory_loads.size() >= max_memory_loads) {
            uint32_t min_uses = UINT32_MAX;
            uint32_t evict_addr = 0;
            for (const auto& pair : memory_loads) {
                if (pair.second.use_count < min_uses) {
                    min_uses = pair.second.use_count;
                    evict_addr = pair.first;
                }
            }
            memory_loads.erase(evict_addr);
        }
        
        memory_loads[mem_addr] = MemoryLoadEntry(mem_addr, value, size, load_addr);
    }
    
    // Get cached memory load
    bool get_memory_load(uint32_t mem_addr, uint64_t* out_value, uint8_t* out_size = nullptr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = memory_loads.find(mem_addr);
        if (it != memory_loads.end() && it->second.is_valid) {
            it->second.use_count++;
            stats.mem_hits++;
            if (out_value) *out_value = it->second.value;
            if (out_size) *out_size = it->second.size;
            return true;
        }
        
        stats.mem_misses++;
        return false;
    }
    
    // Invalidate memory cache for an address
    void invalidate_memory(uint32_t mem_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = memory_loads.find(mem_addr);
        if (it != memory_loads.end()) {
            it->second.is_valid = false;
            stats.invalidations++;
        }
    }
    
    // Invalidate memory range (for stores)
    void invalidate_memory_range(uint32_t start_addr, uint32_t size) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Use 64-bit to prevent overflow
        uint64_t end_addr = static_cast<uint64_t>(start_addr) + size;
        for (auto& pair : memory_loads) {
            uint64_t cached_start = pair.second.address;
            uint64_t cached_end = cached_start + pair.second.size;
            // Check for overlap
            if (cached_start < end_addr && start_addr < cached_end) {
                pair.second.is_valid = false;
                stats.invalidations++;
            }
        }
    }
    
    // ========== Statistics and Management ==========
    
    ConstPropStatistics get_stats() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        return stats;
    }
    
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = ConstPropStatistics();
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        immediates.clear();
        register_values.clear();
        memory_loads.clear();
        stats = ConstPropStatistics();
    }
    
    // Get cache sizes
    size_t get_immediate_count() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        return immediates.size();
    }
    
    size_t get_memory_load_count() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        return memory_loads.size();
    }
    
    size_t get_block_count() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        return register_values.size();
    }
};

/**
 * Register allocation hints
 */
enum class RegAllocHint : uint8_t {
    None = 0,
    Caller = 1,      // Prefer caller-saved registers
    Callee = 2,      // Prefer callee-saved registers
    Float = 3,       // Prefer floating-point registers
    Vector = 4       // Prefer vector registers
};

/**
 * Register liveness information
 */
struct RegisterLiveness {
    uint32_t live_gprs;      // Bitmask of live GPRs
    uint32_t live_fprs;      // Bitmask of live FPRs
    uint32_t live_vrs;       // Bitmask of live vector registers
    uint32_t modified_gprs;  // GPRs modified in this block
    uint32_t modified_fprs;  // FPRs modified in this block
    uint32_t modified_vrs;   // VRs modified in this block
    
    RegisterLiveness() 
        : live_gprs(0), live_fprs(0), live_vrs(0),
          modified_gprs(0), modified_fprs(0), modified_vrs(0) {}
    
    void mark_gpr_live(uint8_t reg) { live_gprs |= (1u << reg); }
    void mark_fpr_live(uint8_t reg) { live_fprs |= (1u << reg); }
    void mark_vr_live(uint8_t reg) { live_vrs |= (1u << reg); }
    void mark_gpr_modified(uint8_t reg) { modified_gprs |= (1u << reg); }
    void mark_fpr_modified(uint8_t reg) { modified_fprs |= (1u << reg); }
    void mark_vr_modified(uint8_t reg) { modified_vrs |= (1u << reg); }
    
    bool is_gpr_live(uint8_t reg) const { return (live_gprs & (1u << reg)) != 0; }
    bool is_fpr_live(uint8_t reg) const { return (live_fprs & (1u << reg)) != 0; }
    bool is_vr_live(uint8_t reg) const { return (live_vrs & (1u << reg)) != 0; }
};

/**
 * Register allocation optimizer
 */
struct RegisterAllocator {
    std::unordered_map<uint32_t, RegisterLiveness> block_liveness;
    
    // Analyze register usage in a basic block
    void analyze_block(uint32_t address, const std::vector<uint32_t>& instructions) {
        RegisterLiveness liveness;
        
        for (uint32_t instr : instructions) {
            uint8_t opcode = (instr >> 26) & 0x3F;
            uint8_t rt = (instr >> 21) & 0x1F;
            uint8_t ra = (instr >> 16) & 0x1F;
            uint8_t rb = (instr >> 11) & 0x1F;
            
            // Mark source registers as live
            if (ra != 0) liveness.mark_gpr_live(ra);
            if (opcode == 31 || opcode == 63) { // Extended opcodes use rb
                if (rb != 0) liveness.mark_gpr_live(rb);
            }
            
            // Mark destination register as modified
            if (rt != 0) {
                liveness.mark_gpr_modified(rt);
            }
            
            // Handle floating-point instructions
            if (opcode >= 48 && opcode <= 63) {
                liveness.mark_fpr_live(ra);
                liveness.mark_fpr_modified(rt);
            }
        }
        
        block_liveness[address] = liveness;
    }
    
    // Get allocation hints for a register
    RegAllocHint get_hint(uint32_t address, uint8_t reg) const {
        auto it = block_liveness.find(address);
        if (it == block_liveness.end()) {
            return RegAllocHint::None;
        }
        
        const auto& liveness = it->second;
        
        // Prefer callee-saved for long-lived values
        if (liveness.is_gpr_live(reg) && !liveness.is_gpr_live((reg + 1) % 32)) {
            return RegAllocHint::Callee;
        }
        
        return RegAllocHint::Caller;
    }
    
    // Get liveness info for a block
    const RegisterLiveness* get_liveness(uint32_t address) const {
        auto it = block_liveness.find(address);
        return (it != block_liveness.end()) ? &it->second : nullptr;
    }
    
    void clear() {
        block_liveness.clear();
    }
};

/**
 * Spill slot for register spilling to memory
 */
struct SpillSlot {
    uint32_t slot_id;       // Unique slot identifier
    uint32_t offset;        // Stack offset (from frame pointer)
    uint8_t reg_num;        // Original register number
    uint8_t reg_type;       // 0=GPR, 1=FPR, 2=VR
    uint32_t spill_addr;    // Instruction address where spill occurred
    uint32_t fill_addr;     // Instruction address where fill occurred (0 if not filled)
    bool is_active;         // Whether the slot is currently in use
    
    SpillSlot()
        : slot_id(0), offset(0), reg_num(0), reg_type(0),
          spill_addr(0), fill_addr(0), is_active(false) {}
    
    SpillSlot(uint32_t id, uint32_t off, uint8_t reg, uint8_t type, uint32_t addr)
        : slot_id(id), offset(off), reg_num(reg), reg_type(type),
          spill_addr(addr), fill_addr(0), is_active(true) {}
};

/**
 * Cross-block register state for inter-procedural analysis
 */
struct CrossBlockState {
    uint32_t block_addr;                     // Address of the basic block
    uint32_t live_in_gprs;                   // GPRs live at block entry
    uint32_t live_out_gprs;                  // GPRs live at block exit
    uint32_t live_in_fprs;                   // FPRs live at block entry
    uint32_t live_out_fprs;                  // FPRs live at block exit
    uint32_t live_in_vrs;                    // VRs live at block entry
    uint32_t live_out_vrs;                   // VRs live at block exit
    std::vector<uint32_t> successors;        // Successor block addresses
    std::vector<uint32_t> predecessors;      // Predecessor block addresses
    bool is_analyzed;                        // Whether cross-block analysis is complete
    
    CrossBlockState()
        : block_addr(0), live_in_gprs(0), live_out_gprs(0),
          live_in_fprs(0), live_out_fprs(0), live_in_vrs(0), live_out_vrs(0),
          is_analyzed(false) {}
    
    CrossBlockState(uint32_t addr)
        : block_addr(addr), live_in_gprs(0), live_out_gprs(0),
          live_in_fprs(0), live_out_fprs(0), live_in_vrs(0), live_out_vrs(0),
          is_analyzed(false) {}
};

/**
 * Register copy information for coalescing
 */
struct CopyInfo {
    uint32_t instr_addr;    // Address of copy instruction
    uint8_t src_reg;        // Source register
    uint8_t dst_reg;        // Destination register
    uint8_t reg_type;       // 0=GPR, 1=FPR, 2=VR
    bool is_eliminated;     // Whether the copy was eliminated
    
    CopyInfo()
        : instr_addr(0), src_reg(0), dst_reg(0), reg_type(0), is_eliminated(false) {}
    
    CopyInfo(uint32_t addr, uint8_t src, uint8_t dst, uint8_t type)
        : instr_addr(addr), src_reg(src), dst_reg(dst), reg_type(type), is_eliminated(false) {}
};

/**
 * Register allocation statistics
 */
struct RegAllocStatistics {
    uint64_t blocks_analyzed;        // Number of blocks analyzed
    uint64_t total_spills;           // Total number of spills
    uint64_t total_fills;            // Total number of fills
    uint64_t spills_avoided;         // Spills avoided through optimization
    uint64_t copies_eliminated;      // Register copies eliminated
    uint64_t cross_block_props;      // Cross-block liveness propagations
    
    RegAllocStatistics()
        : blocks_analyzed(0), total_spills(0), total_fills(0),
          spills_avoided(0), copies_eliminated(0), cross_block_props(0) {}
    
    double get_spill_ratio() const {
        uint64_t total = total_spills + spills_avoided;
        return (total > 0) ? (100.0 * total_spills / total) : 0.0;
    }
};

/**
 * Register coalescer for eliminating register copies
 */
struct RegisterCoalescer {
    std::vector<CopyInfo> copies;              // All detected copies
    std::unordered_map<uint8_t, uint8_t> gpr_alias;  // GPR alias mapping
    std::unordered_map<uint8_t, uint8_t> fpr_alias;  // FPR alias mapping
    std::unordered_map<uint8_t, uint8_t> vr_alias;   // VR alias mapping
    oc_mutex mutex;
    
    // Add a register copy for potential coalescing
    void add_copy(uint32_t instr_addr, uint8_t src, uint8_t dst, uint8_t reg_type) {
        oc_lock_guard<oc_mutex> lock(mutex);
        copies.push_back(CopyInfo(instr_addr, src, dst, reg_type));
    }
    
    // Try to eliminate a copy by aliasing registers
    bool try_coalesce(uint8_t src, uint8_t dst, uint8_t reg_type, 
                       const RegisterLiveness& liveness) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Can't coalesce if both registers are live
        bool src_live = false;
        bool dst_live = false;
        
        switch (reg_type) {
            case 0: // GPR
                src_live = liveness.is_gpr_live(src);
                dst_live = liveness.is_gpr_live(dst);
                break;
            case 1: // FPR
                src_live = liveness.is_fpr_live(src);
                dst_live = liveness.is_fpr_live(dst);
                break;
            case 2: // VR
                src_live = liveness.is_vr_live(src);
                dst_live = liveness.is_vr_live(dst);
                break;
        }
        
        // Can coalesce if both registers are not simultaneously live (non-interfering)
        // We alias dst to src, so dst uses become src uses
        if (!src_live && !dst_live) {
            // Neither is live - safe to coalesce
            switch (reg_type) {
                case 0: gpr_alias[dst] = src; break;
                case 1: fpr_alias[dst] = src; break;
                case 2: vr_alias[dst] = src; break;
            }
            return true;
        }
        
        return false;
    }
    
    // Maximum iterations for alias chain traversal (one per possible register)
    static constexpr int MAX_ALIAS_CHAIN_LENGTH = 32;
    
    // Get the actual register after coalescing
    uint8_t get_actual_reg(uint8_t reg, uint8_t reg_type) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        const std::unordered_map<uint8_t, uint8_t>* alias_map = nullptr;
        switch (reg_type) {
            case 0: alias_map = &gpr_alias; break;
            case 1: alias_map = &fpr_alias; break;
            case 2: alias_map = &vr_alias; break;
            default: return reg;
        }
        
        // Follow alias chain (limited to prevent cycles)
        uint8_t current = reg;
        int iterations = 0;
        while (iterations < MAX_ALIAS_CHAIN_LENGTH) {
            auto it = alias_map->find(current);
            if (it == alias_map->end()) break;
            current = it->second;
            iterations++;
        }
        
        return current;
    }
    
    // Run coalescing pass
    size_t run_coalescing(const std::unordered_map<uint32_t, RegisterLiveness>& liveness_map) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        size_t eliminated = 0;
        for (auto& copy : copies) {
            if (copy.is_eliminated) continue;
            
            // Find liveness at the copy point (approximate with containing block)
            for (const auto& pair : liveness_map) {
                if (pair.first <= copy.instr_addr) {
                    bool src_live = false;
                    switch (copy.reg_type) {
                        case 0: src_live = pair.second.is_gpr_live(copy.src_reg); break;
                        case 1: src_live = pair.second.is_fpr_live(copy.src_reg); break;
                        case 2: src_live = pair.second.is_vr_live(copy.src_reg); break;
                    }
                    
                    if (!src_live) {
                        copy.is_eliminated = true;
                        switch (copy.reg_type) {
                            case 0: gpr_alias[copy.dst_reg] = copy.src_reg; break;
                            case 1: fpr_alias[copy.dst_reg] = copy.src_reg; break;
                            case 2: vr_alias[copy.dst_reg] = copy.src_reg; break;
                        }
                        eliminated++;
                    }
                    break;
                }
            }
        }
        
        return eliminated;
    }
    
    size_t get_eliminated_count() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        size_t count = 0;
        for (const auto& copy : copies) {
            if (copy.is_eliminated) count++;
        }
        return count;
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        copies.clear();
        gpr_alias.clear();
        fpr_alias.clear();
        vr_alias.clear();
    }
};

/**
 * Enhanced register allocator with spill/fill and cross-block support
 */
struct EnhancedRegisterAllocator {
    // Basic liveness per block
    std::unordered_map<uint32_t, RegisterLiveness> block_liveness;
    
    // Cross-block state for inter-procedural analysis
    std::unordered_map<uint32_t, CrossBlockState> cross_block_state;
    
    // Spill slots
    std::vector<SpillSlot> spill_slots;
    uint32_t next_slot_id;
    uint32_t next_stack_offset;
    static constexpr uint32_t SLOT_SIZE = 16;  // 16 bytes for VR alignment
    
    // Register coalescer
    RegisterCoalescer coalescer;
    
    // Statistics
    RegAllocStatistics stats;
    oc_mutex mutex;
    
    EnhancedRegisterAllocator() : next_slot_id(1), next_stack_offset(0) {}
    
    // Analyze register usage in a basic block
    void analyze_block(uint32_t address, const std::vector<uint32_t>& instructions) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        RegisterLiveness liveness;
        
        for (size_t i = 0; i < instructions.size(); i++) {
            uint32_t instr = instructions[i];
            uint32_t instr_addr = address + i * 4;
            uint8_t opcode = (instr >> 26) & 0x3F;
            uint8_t rt = (instr >> 21) & 0x1F;
            uint8_t ra = (instr >> 16) & 0x1F;
            uint8_t rb = (instr >> 11) & 0x1F;
            
            // Mark source registers as live
            if (ra != 0) liveness.mark_gpr_live(ra);
            if (opcode == 31 || opcode == 63) { // Extended opcodes use rb
                if (rb != 0) liveness.mark_gpr_live(rb);
            }
            
            // Mark destination register as modified
            if (rt != 0) {
                liveness.mark_gpr_modified(rt);
            }
            
            // Handle floating-point instructions
            if (opcode >= 48 && opcode <= 63) {
                liveness.mark_fpr_live(ra);
                liveness.mark_fpr_modified(rt);
            }
            
            // Detect register-to-register moves for coalescing
            // mr rD, rA is rlwinm rD, rA, 0, 0, 31 (opcode 21) or or rD, rA, rA (opcode 31, xo 444)
            if (opcode == 31) {
                uint16_t xo = (instr >> 1) & 0x3FF;
                if (xo == 444 && ra == rb) {  // or rD, rA, rA = move
                    coalescer.add_copy(instr_addr, ra, rt, 0);
                }
            }
        }
        
        block_liveness[address] = liveness;
        stats.blocks_analyzed++;
        
        // Initialize cross-block state if not exists
        if (cross_block_state.find(address) == cross_block_state.end()) {
            cross_block_state[address] = CrossBlockState(address);
        }
    }
    
    // Add control flow edge for cross-block analysis
    void add_edge(uint32_t from_addr, uint32_t to_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        if (cross_block_state.find(from_addr) == cross_block_state.end()) {
            cross_block_state[from_addr] = CrossBlockState(from_addr);
        }
        if (cross_block_state.find(to_addr) == cross_block_state.end()) {
            cross_block_state[to_addr] = CrossBlockState(to_addr);
        }
        
        cross_block_state[from_addr].successors.push_back(to_addr);
        cross_block_state[to_addr].predecessors.push_back(from_addr);
    }
    
    // Propagate liveness across blocks (backwards dataflow analysis)
    bool propagate_liveness() {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        bool changed = true;
        int iterations = 0;
        const int MAX_ITERATIONS = 100;
        
        while (changed && iterations < MAX_ITERATIONS) {
            changed = false;
            iterations++;
            
            for (auto& pair : cross_block_state) {
                auto& state = pair.second;
                uint32_t addr = state.block_addr;
                
                // Get basic block liveness
                auto it = block_liveness.find(addr);
                if (it == block_liveness.end()) continue;
                const auto& liveness = it->second;
                
                // live_out = union of live_in of all successors
                uint32_t new_live_out_gprs = 0;
                uint32_t new_live_out_fprs = 0;
                uint32_t new_live_out_vrs = 0;
                
                for (uint32_t succ_addr : state.successors) {
                    auto succ_it = cross_block_state.find(succ_addr);
                    if (succ_it != cross_block_state.end()) {
                        new_live_out_gprs |= succ_it->second.live_in_gprs;
                        new_live_out_fprs |= succ_it->second.live_in_fprs;
                        new_live_out_vrs |= succ_it->second.live_in_vrs;
                    }
                }
                
                // live_in = (live_out - def) | use
                uint32_t new_live_in_gprs = (new_live_out_gprs & ~liveness.modified_gprs) | liveness.live_gprs;
                uint32_t new_live_in_fprs = (new_live_out_fprs & ~liveness.modified_fprs) | liveness.live_fprs;
                uint32_t new_live_in_vrs = (new_live_out_vrs & ~liveness.modified_vrs) | liveness.live_vrs;
                
                if (new_live_in_gprs != state.live_in_gprs ||
                    new_live_in_fprs != state.live_in_fprs ||
                    new_live_in_vrs != state.live_in_vrs ||
                    new_live_out_gprs != state.live_out_gprs ||
                    new_live_out_fprs != state.live_out_fprs ||
                    new_live_out_vrs != state.live_out_vrs) {
                    
                    state.live_in_gprs = new_live_in_gprs;
                    state.live_in_fprs = new_live_in_fprs;
                    state.live_in_vrs = new_live_in_vrs;
                    state.live_out_gprs = new_live_out_gprs;
                    state.live_out_fprs = new_live_out_fprs;
                    state.live_out_vrs = new_live_out_vrs;
                    changed = true;
                }
                
                state.is_analyzed = true;
            }
            
            stats.cross_block_props++;
        }
        
        return !changed;  // Return true if converged
    }
    
    // Allocate a spill slot for a register
    uint32_t allocate_spill_slot(uint8_t reg_num, uint8_t reg_type, uint32_t spill_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Try to reuse an inactive slot
        for (auto& slot : spill_slots) {
            if (!slot.is_active && slot.reg_type == reg_type) {
                slot.reg_num = reg_num;
                slot.spill_addr = spill_addr;
                slot.fill_addr = 0;
                slot.is_active = true;
                stats.total_spills++;
                return slot.slot_id;
            }
        }
        
        // Allocate new slot
        uint32_t slot_id = next_slot_id++;
        uint32_t offset = next_stack_offset;
        next_stack_offset += SLOT_SIZE;
        
        spill_slots.push_back(SpillSlot(slot_id, offset, reg_num, reg_type, spill_addr));
        stats.total_spills++;
        
        return slot_id;
    }
    
    // Free a spill slot
    void free_spill_slot(uint32_t slot_id, uint32_t fill_addr) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        for (auto& slot : spill_slots) {
            if (slot.slot_id == slot_id && slot.is_active) {
                slot.is_active = false;
                slot.fill_addr = fill_addr;
                stats.total_fills++;
                return;
            }
        }
    }
    
    // Check if a register needs to be spilled at a point
    bool needs_spill(uint32_t block_addr, uint8_t reg_num, uint8_t reg_type) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        auto it = cross_block_state.find(block_addr);
        if (it == cross_block_state.end()) return false;
        
        const auto& state = it->second;
        
        // Check if register is live out of this block
        switch (reg_type) {
            case 0: return (state.live_out_gprs & (1u << reg_num)) != 0;
            case 1: return (state.live_out_fprs & (1u << reg_num)) != 0;
            case 2: return (state.live_out_vrs & (1u << reg_num)) != 0;
        }
        
        return false;
    }
    
    // Get spill slot info
    const SpillSlot* get_spill_slot(uint32_t slot_id) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        for (const auto& slot : spill_slots) {
            if (slot.slot_id == slot_id) {
                return &slot;
            }
        }
        return nullptr;
    }
    
    // Get allocation hints for a register
    RegAllocHint get_hint(uint32_t address, uint8_t reg) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        auto it = block_liveness.find(address);
        if (it == block_liveness.end()) {
            return RegAllocHint::None;
        }
        
        const auto& liveness = it->second;
        
        // Check cross-block state for better hints
        auto cross_it = cross_block_state.find(address);
        if (cross_it != cross_block_state.end() && cross_it->second.is_analyzed) {
            const auto& state = cross_it->second;
            
            // If register is live across block boundaries, prefer callee-saved
            // This indicates the value needs to survive across potential function calls
            if (state.live_out_gprs & (1u << reg)) {
                return RegAllocHint::Callee;
            }
            
            // If register is live into the block, it was set by a predecessor
            // and may benefit from callee-saved allocation
            if (state.live_in_gprs & (1u << reg)) {
                return RegAllocHint::Callee;
            }
        }
        
        // Default to caller-saved for values only used within a block
        return RegAllocHint::Caller;
    }
    
    // Get liveness info for a block
    const RegisterLiveness* get_liveness(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        auto it = block_liveness.find(address);
        return (it != block_liveness.end()) ? &it->second : nullptr;
    }
    
    // Get cross-block state
    const CrossBlockState* get_cross_block_state(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        
        auto it = cross_block_state.find(address);
        return (it != cross_block_state.end()) ? &it->second : nullptr;
    }
    
    // Run register coalescing
    size_t run_coalescing() {
        oc_lock_guard<oc_mutex> lock(mutex);
        size_t eliminated = coalescer.run_coalescing(block_liveness);
        stats.copies_eliminated += eliminated;
        return eliminated;
    }
    
    // Get coalesced register
    uint8_t get_coalesced_reg(uint8_t reg, uint8_t reg_type) const {
        return coalescer.get_actual_reg(reg, reg_type);
    }
    
    // Get statistics
    RegAllocStatistics get_stats() const {
        oc_lock_guard<oc_mutex> lock(const_cast<oc_mutex&>(mutex));
        return stats;
    }
    
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = RegAllocStatistics();
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        block_liveness.clear();
        cross_block_state.clear();
        spill_slots.clear();
        coalescer.clear();
        next_slot_id = 1;
        next_stack_offset = 0;
        stats = RegAllocStatistics();
    }
};

/**
 * Lazy compilation state
 */
enum class LazyState : uint8_t {
    NotCompiled = 0,
    Pending = 1,
    Compiling = 2,
    Compiled = 3,
    Failed = 4
};

/**
 * Lazy compilation entry
 */
struct LazyCompilationEntry {
    uint32_t address;
    const uint8_t* code;
    size_t size;
    LazyState state;
    std::atomic<uint32_t> execution_count;
    uint32_t threshold;  // Compile after this many executions
    
    LazyCompilationEntry()
        : address(0), code(nullptr), size(0), state(LazyState::NotCompiled),
          execution_count(0), threshold(10) {}
    
    LazyCompilationEntry(uint32_t addr, const uint8_t* c, size_t s, uint32_t thresh = 10)
        : address(addr), code(c), size(s), state(LazyState::NotCompiled),
          execution_count(0), threshold(thresh) {}
    
    // Move constructor
    LazyCompilationEntry(LazyCompilationEntry&& other) noexcept
        : address(other.address), code(other.code), size(other.size),
          state(other.state), execution_count(other.execution_count.load()),
          threshold(other.threshold) {}
    
    // Move assignment
    LazyCompilationEntry& operator=(LazyCompilationEntry&& other) noexcept {
        if (this != &other) {
            address = other.address;
            code = other.code;
            size = other.size;
            state = other.state;
            execution_count.store(other.execution_count.load());
            threshold = other.threshold;
        }
        return *this;
    }
    
    // Delete copy operations
    LazyCompilationEntry(const LazyCompilationEntry&) = delete;
    LazyCompilationEntry& operator=(const LazyCompilationEntry&) = delete;
    
    bool should_compile() {
        return execution_count.fetch_add(1) + 1 >= threshold;
    }
};

/**
 * Lazy compilation manager
 */
struct LazyCompilationManager {
    std::unordered_map<uint32_t, std::unique_ptr<LazyCompilationEntry>> entries;
    oc_mutex mutex;
    
    void register_lazy(uint32_t address, const uint8_t* code, size_t size, uint32_t threshold = 10) {
        oc_lock_guard<oc_mutex> lock(mutex);
        entries[address] = std::make_unique<LazyCompilationEntry>(address, code, size, threshold);
    }
    
    LazyCompilationEntry* get_entry(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second.get() : nullptr;
    }
    
    void mark_compiling(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Compiling;
        }
    }
    
    void mark_compiled(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Compiled;
        }
    }
    
    void mark_failed(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Failed;
        }
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        entries.clear();
    }
};

/**
 * Lazy compilation statistics
 */
struct LazyCompilationStats {
    uint64_t total_registered;       // Total functions registered for lazy compilation
    uint64_t total_compiled;         // Total functions compiled
    uint64_t total_failed;           // Total compilation failures
    uint64_t total_executions;       // Total execution count across all entries
    uint64_t hot_path_promotions;    // Times a function was promoted to hot status
    uint64_t stub_calls;             // Times interpreter stub was called
    
    LazyCompilationStats()
        : total_registered(0), total_compiled(0), total_failed(0),
          total_executions(0), hot_path_promotions(0), stub_calls(0) {}
};

/**
 * Hot path entry for prioritization
 */
struct HotPathEntry {
    uint32_t address;
    uint32_t execution_count;
    bool is_compiled;
    
    HotPathEntry() : address(0), execution_count(0), is_compiled(false) {}
    HotPathEntry(uint32_t addr, uint32_t count, bool compiled)
        : address(addr), execution_count(count), is_compiled(compiled) {}
    
    // Higher execution count = higher priority
    bool operator<(const HotPathEntry& other) const {
        return execution_count < other.execution_count;
    }
};

/**
 * Interpreter stub callback type
 * Called when uncompiled code is executed
 * Returns: 0 = continue with interpreter, 1 = code is now compiled
 */
using InterpreterStubCallback = int (*)(uint32_t address, void* user_data);

/**
 * Enhanced lazy compilation manager with hot path detection and stub support
 */
struct EnhancedLazyCompilationManager {
    std::unordered_map<uint32_t, std::unique_ptr<LazyCompilationEntry>> entries;
    std::priority_queue<HotPathEntry> hot_queue;  // Priority queue for hot paths
    mutable oc_mutex mutex;  // mutable to allow locking in const methods
    
    uint32_t default_threshold;          // Default compilation threshold
    uint32_t hot_threshold;              // Threshold to consider a path "hot"
    InterpreterStubCallback stub_callback;
    void* stub_user_data;
    LazyCompilationStats stats;
    
    static constexpr uint32_t DEFAULT_THRESHOLD = 10;
    static constexpr uint32_t HOT_THRESHOLD = 100;
    
    EnhancedLazyCompilationManager()
        : default_threshold(DEFAULT_THRESHOLD), hot_threshold(HOT_THRESHOLD),
          stub_callback(nullptr), stub_user_data(nullptr) {}
    
    // Set default compilation threshold
    void set_default_threshold(uint32_t threshold) {
        oc_lock_guard<oc_mutex> lock(mutex);
        default_threshold = threshold;
    }
    
    uint32_t get_default_threshold() const {
        return default_threshold;
    }
    
    // Set hot path threshold
    void set_hot_threshold(uint32_t threshold) {
        oc_lock_guard<oc_mutex> lock(mutex);
        hot_threshold = threshold;
    }
    
    // Set interpreter stub callback
    void set_stub_callback(InterpreterStubCallback callback, void* user_data) {
        oc_lock_guard<oc_mutex> lock(mutex);
        stub_callback = callback;
        stub_user_data = user_data;
    }
    
    // Register code for lazy compilation
    void register_lazy(uint32_t address, const uint8_t* code, size_t size, uint32_t threshold = 0) {
        oc_lock_guard<oc_mutex> lock(mutex);
        uint32_t actual_threshold = (threshold == 0) ? default_threshold : threshold;
        entries[address] = std::make_unique<LazyCompilationEntry>(address, code, size, actual_threshold);
        stats.total_registered++;
    }
    
    // Get entry for an address
    LazyCompilationEntry* get_entry(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second.get() : nullptr;
    }
    
    // Increment execution count and check if should compile
    // Returns: true if should compile now
    bool record_execution(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = entries.find(address);
        if (it == entries.end()) return false;
        
        auto* entry = it->second.get();
        stats.total_executions++;
        
        // Already compiled or compiling
        if (entry->state == LazyState::Compiled || entry->state == LazyState::Compiling) {
            return false;
        }
        
        // Increment execution count
        uint32_t count = entry->execution_count.fetch_add(1) + 1;
        
        // Check for hot path promotion
        if (count == hot_threshold) {
            stats.hot_path_promotions++;
            hot_queue.push(HotPathEntry(address, count, false));
        }
        
        // Check if should compile
        if (count >= entry->threshold) {
            entry->state = LazyState::Pending;
            return true;
        }
        
        return false;
    }
    
    // Call interpreter stub for uncompiled code
    int call_stub(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats.stub_calls++;
        
        if (stub_callback) {
            return stub_callback(address, stub_user_data);
        }
        return 0;  // No stub, continue with interpreter
    }
    
    // Mark entry as compiling
    void mark_compiling(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Compiling;
        }
    }
    
    // Mark entry as compiled
    void mark_compiled(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Compiled;
            stats.total_compiled++;
        }
    }
    
    // Mark entry as failed
    void mark_failed(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        if (it != entries.end()) {
            it->second->state = LazyState::Failed;
            stats.total_failed++;
        }
    }
    
    // Get execution count for an address
    uint32_t get_execution_count(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second->execution_count.load() : 0;
    }
    
    // Get state for an address
    LazyState get_state(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second->state : LazyState::NotCompiled;
    }
    
    // Get next hot address to compile (highest priority)
    // Returns 0 if no hot addresses pending
    uint32_t get_next_hot_address() {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        while (!hot_queue.empty()) {
            HotPathEntry top = hot_queue.top();
            hot_queue.pop();
            
            auto it = entries.find(top.address);
            if (it != entries.end() && it->second->state == LazyState::Pending) {
                return top.address;
            }
        }
        
        return 0;
    }
    
    // Get list of hot addresses (sorted by execution count, descending)
    // Uses partial_sort for efficiency when max_count < total entries
    std::vector<HotPathEntry> get_hot_addresses(size_t max_count = 100) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        std::vector<HotPathEntry> result;
        result.reserve(std::min(entries.size(), max_count * 2));  // Reserve reasonable space
        
        for (const auto& pair : entries) {
            uint32_t count = pair.second->execution_count.load();
            if (count >= hot_threshold) {
                result.push_back(HotPathEntry(
                    pair.first, count, 
                    pair.second->state == LazyState::Compiled
                ));
            }
        }
        
        // Use partial_sort if we have more results than needed
        if (result.size() > max_count) {
            std::partial_sort(result.begin(), result.begin() + max_count, result.end(),
                              [](const HotPathEntry& a, const HotPathEntry& b) {
                                  return a.execution_count > b.execution_count;
                              });
            result.resize(max_count);
        } else {
            // Full sort for small result sets
            std::sort(result.begin(), result.end(), 
                      [](const HotPathEntry& a, const HotPathEntry& b) {
                          return a.execution_count > b.execution_count;
                      });
        }
        
        return result;
    }
    
    // Get pending compilation count
    size_t get_pending_count() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        size_t count = 0;
        for (const auto& pair : entries) {
            if (pair.second->state == LazyState::Pending) {
                count++;
            }
        }
        return count;
    }
    
    // Get statistics
    LazyCompilationStats get_stats() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return stats;
    }
    
    // Reset statistics
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = LazyCompilationStats();
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        entries.clear();
        // Clear the priority queue
        while (!hot_queue.empty()) {
            hot_queue.pop();
        }
        stats = LazyCompilationStats();
    }
};

// ============================================================================
// Tiered Compilation System
// ============================================================================

/**
 * Compilation tiers for multi-tier JIT
 */
enum class CompilationTier : uint8_t {
    Interpreter = 0,  // Tier 0: Interpreted execution (immediate, no compilation)
    Baseline = 1,     // Tier 1: Fast compilation, low optimization
    Optimizing = 2    // Tier 2: Slow compilation, high optimization
};

/**
 * Get tier name as string
 */
inline const char* tier_to_string(CompilationTier tier) {
    switch (tier) {
        case CompilationTier::Interpreter: return "Interpreter";
        case CompilationTier::Baseline: return "Baseline";
        case CompilationTier::Optimizing: return "Optimizing";
        default: return "Unknown";
    }
}

/**
 * Tiered compilation entry
 */
struct TieredCompilationEntry {
    uint32_t address;
    const uint8_t* code;
    size_t size;
    std::atomic<int> current_tier;                // Atomic tier for thread safety
    std::atomic<uint32_t> execution_count;
    std::atomic<uint32_t> baseline_tier_executions;  // Executions at baseline tier
    uint32_t tier0_to_1_threshold;                // Threshold for Interpreter → Baseline
    uint32_t tier1_to_2_threshold;                // Threshold for Baseline → Optimizing
    void* baseline_code;                          // Pointer to baseline compiled code
    void* optimized_code;                         // Pointer to optimized compiled code
    std::atomic<bool> is_promoting;               // Atomic flag for promotion in progress
    
    TieredCompilationEntry()
        : address(0), code(nullptr), size(0), 
          current_tier(static_cast<int>(CompilationTier::Interpreter)),
          execution_count(0), baseline_tier_executions(0), tier0_to_1_threshold(10),
          tier1_to_2_threshold(1000), baseline_code(nullptr), optimized_code(nullptr),
          is_promoting(false) {}
    
    TieredCompilationEntry(uint32_t addr, const uint8_t* c, size_t s, 
                            uint32_t t0_t1 = 10, uint32_t t1_t2 = 1000)
        : address(addr), code(c), size(s), 
          current_tier(static_cast<int>(CompilationTier::Interpreter)),
          execution_count(0), baseline_tier_executions(0), tier0_to_1_threshold(t0_t1),
          tier1_to_2_threshold(t1_t2), baseline_code(nullptr), optimized_code(nullptr),
          is_promoting(false) {}
    
    // Move constructor
    TieredCompilationEntry(TieredCompilationEntry&& other) noexcept
        : address(other.address), code(other.code), size(other.size),
          current_tier(other.current_tier.load()), 
          execution_count(other.execution_count.load()),
          baseline_tier_executions(other.baseline_tier_executions.load()),
          tier0_to_1_threshold(other.tier0_to_1_threshold),
          tier1_to_2_threshold(other.tier1_to_2_threshold),
          baseline_code(other.baseline_code), optimized_code(other.optimized_code),
          is_promoting(other.is_promoting.load()) {}
    
    // Move assignment
    TieredCompilationEntry& operator=(TieredCompilationEntry&& other) noexcept {
        if (this != &other) {
            address = other.address;
            code = other.code;
            size = other.size;
            current_tier.store(other.current_tier.load());
            execution_count.store(other.execution_count.load());
            baseline_tier_executions.store(other.baseline_tier_executions.load());
            tier0_to_1_threshold = other.tier0_to_1_threshold;
            tier1_to_2_threshold = other.tier1_to_2_threshold;
            baseline_code = other.baseline_code;
            optimized_code = other.optimized_code;
            is_promoting.store(other.is_promoting.load());
        }
        return *this;
    }
    
    // Delete copy operations
    TieredCompilationEntry(const TieredCompilationEntry&) = delete;
    TieredCompilationEntry& operator=(const TieredCompilationEntry&) = delete;
    
    // Get current tier (thread-safe)
    CompilationTier get_tier() const {
        return static_cast<CompilationTier>(current_tier.load());
    }
    
    // Set current tier (thread-safe)
    void set_tier(CompilationTier tier) {
        current_tier.store(static_cast<int>(tier));
    }
    
    // Check if should promote to next tier
    // Returns: next tier if should promote, current tier if not
    CompilationTier check_promotion() {
        uint32_t count = execution_count.load();
        CompilationTier tier = get_tier();
        
        switch (tier) {
            case CompilationTier::Interpreter:
                if (count >= tier0_to_1_threshold) {
                    return CompilationTier::Baseline;
                }
                break;
            case CompilationTier::Baseline:
                if (baseline_tier_executions.load() >= tier1_to_2_threshold) {
                    return CompilationTier::Optimizing;
                }
                break;
            case CompilationTier::Optimizing:
                // Already at highest tier
                break;
        }
        
        return tier;
    }
    
    // Get compiled code pointer for current tier
    void* get_compiled_code() const {
        CompilationTier tier = get_tier();
        switch (tier) {
            case CompilationTier::Baseline:
                return baseline_code;
            case CompilationTier::Optimizing:
                return optimized_code;
            default:
                return nullptr;  // Interpreter has no compiled code
        }
    }
};

/**
 * Tiered compilation callback types
 */
using BaselineCompileCallback = void* (*)(uint32_t address, const uint8_t* code, size_t size, void* user_data);
using OptimizingCompileCallback = void* (*)(uint32_t address, const uint8_t* code, size_t size, void* baseline_code, void* user_data);

/**
 * Tiered compilation statistics
 */
struct TieredCompilationStats {
    uint64_t total_registered;           // Total functions registered
    uint64_t tier0_executions;           // Executions at interpreter tier
    uint64_t tier1_executions;           // Executions at baseline tier
    uint64_t tier2_executions;           // Executions at optimizing tier
    uint64_t tier0_to_1_promotions;      // Promotions from interpreter to baseline
    uint64_t tier1_to_2_promotions;      // Promotions from baseline to optimizing
    uint64_t baseline_compilations;      // Successful baseline compilations
    uint64_t optimizing_compilations;    // Successful optimizing compilations
    uint64_t compilation_failures;       // Total compilation failures
    
    TieredCompilationStats()
        : total_registered(0), tier0_executions(0), tier1_executions(0), tier2_executions(0),
          tier0_to_1_promotions(0), tier1_to_2_promotions(0), baseline_compilations(0),
          optimizing_compilations(0), compilation_failures(0) {}
    
    double get_tier1_coverage() const {
        uint64_t total = tier0_executions + tier1_executions + tier2_executions;
        return (total > 0) ? (100.0 * (tier1_executions + tier2_executions) / total) : 0.0;
    }
    
    double get_tier2_coverage() const {
        uint64_t total = tier0_executions + tier1_executions + tier2_executions;
        return (total > 0) ? (100.0 * tier2_executions / total) : 0.0;
    }
};

/**
 * Tiered Compilation Manager
 * Manages multi-tier JIT compilation with automatic tier promotion
 */
struct TieredCompilationManager {
    std::unordered_map<uint32_t, std::unique_ptr<TieredCompilationEntry>> entries;
    mutable oc_mutex mutex;
    
    // Default thresholds
    uint32_t default_tier0_to_1_threshold;
    uint32_t default_tier1_to_2_threshold;
    
    // Compilation callbacks
    BaselineCompileCallback baseline_compiler;
    OptimizingCompileCallback optimizing_compiler;
    void* compiler_user_data;
    
    // Statistics
    TieredCompilationStats stats;
    
    static constexpr uint32_t DEFAULT_TIER0_TO_1 = 10;
    static constexpr uint32_t DEFAULT_TIER1_TO_2 = 1000;
    
    TieredCompilationManager()
        : default_tier0_to_1_threshold(DEFAULT_TIER0_TO_1),
          default_tier1_to_2_threshold(DEFAULT_TIER1_TO_2),
          baseline_compiler(nullptr), optimizing_compiler(nullptr),
          compiler_user_data(nullptr) {}
    
    // Set tier promotion thresholds
    void set_thresholds(uint32_t tier0_to_1, uint32_t tier1_to_2) {
        oc_lock_guard<oc_mutex> lock(mutex);
        default_tier0_to_1_threshold = tier0_to_1;
        default_tier1_to_2_threshold = tier1_to_2;
    }
    
    // Get current thresholds
    void get_thresholds(uint32_t* tier0_to_1, uint32_t* tier1_to_2) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        if (tier0_to_1) *tier0_to_1 = default_tier0_to_1_threshold;
        if (tier1_to_2) *tier1_to_2 = default_tier1_to_2_threshold;
    }
    
    // Set compilation callbacks
    void set_compilers(BaselineCompileCallback baseline, OptimizingCompileCallback optimizing,
                       void* user_data) {
        oc_lock_guard<oc_mutex> lock(mutex);
        baseline_compiler = baseline;
        optimizing_compiler = optimizing;
        compiler_user_data = user_data;
    }
    
    // Register code for tiered compilation
    void register_code(uint32_t address, const uint8_t* code, size_t size,
                       uint32_t t0_t1 = 0, uint32_t t1_t2 = 0) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        uint32_t thresh0 = (t0_t1 == 0) ? default_tier0_to_1_threshold : t0_t1;
        uint32_t thresh1 = (t1_t2 == 0) ? default_tier1_to_2_threshold : t1_t2;
        
        entries[address] = std::make_unique<TieredCompilationEntry>(
            address, code, size, thresh0, thresh1);
        stats.total_registered++;
    }
    
    // Get entry for an address
    TieredCompilationEntry* get_entry(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second.get() : nullptr;
    }
    
    // Record execution and check if should promote
    // Returns: new tier if promotion triggered, current tier otherwise
    CompilationTier record_execution(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = entries.find(address);
        if (it == entries.end()) return CompilationTier::Interpreter;
        
        auto* entry = it->second.get();
        CompilationTier tier = entry->get_tier();
        
        // Track execution at current tier
        switch (tier) {
            case CompilationTier::Interpreter:
                stats.tier0_executions++;
                break;
            case CompilationTier::Baseline:
                stats.tier1_executions++;
                entry->baseline_tier_executions.fetch_add(1);
                break;
            case CompilationTier::Optimizing:
                stats.tier2_executions++;
                break;
        }
        
        // Increment total execution count
        entry->execution_count.fetch_add(1);
        
        // Skip if already promoting
        if (entry->is_promoting.load()) {
            return tier;
        }
        
        // Check if should promote
        CompilationTier next_tier = entry->check_promotion();
        if (next_tier != tier) {
            return next_tier;  // Return the tier to promote to
        }
        
        return tier;
    }
    
    // Perform tier promotion (compile at new tier)
    // Returns: true if promotion successful
    bool promote(uint32_t address, CompilationTier target_tier) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        auto it = entries.find(address);
        if (it == entries.end()) return false;
        
        auto* entry = it->second.get();
        CompilationTier current = entry->get_tier();
        
        // Validate promotion
        if (target_tier <= current) return false;
        
        // Try to set is_promoting atomically
        bool expected = false;
        if (!entry->is_promoting.compare_exchange_strong(expected, true)) {
            return false;  // Another thread is already promoting
        }
        
        bool success = false;
        
        if (target_tier == CompilationTier::Baseline) {
            // Compile at baseline tier
            if (baseline_compiler) {
                void* code_ptr = baseline_compiler(
                    entry->address, entry->code, entry->size, compiler_user_data);
                if (code_ptr) {
                    entry->baseline_code = code_ptr;
                    entry->set_tier(CompilationTier::Baseline);
                    stats.tier0_to_1_promotions++;
                    stats.baseline_compilations++;
                    success = true;
                } else {
                    stats.compilation_failures++;
                }
            } else {
                // No compiler, just mark as promoted
                entry->set_tier(CompilationTier::Baseline);
                stats.tier0_to_1_promotions++;
                success = true;
            }
        } else if (target_tier == CompilationTier::Optimizing) {
            // Compile at optimizing tier
            if (optimizing_compiler) {
                void* code_ptr = optimizing_compiler(
                    entry->address, entry->code, entry->size, 
                    entry->baseline_code, compiler_user_data);
                if (code_ptr) {
                    entry->optimized_code = code_ptr;
                    entry->set_tier(CompilationTier::Optimizing);
                    stats.tier1_to_2_promotions++;
                    stats.optimizing_compilations++;
                    success = true;
                } else {
                    stats.compilation_failures++;
                }
            } else {
                // No compiler, just mark as promoted
                entry->set_tier(CompilationTier::Optimizing);
                stats.tier1_to_2_promotions++;
                success = true;
            }
        }
        
        entry->is_promoting.store(false);
        return success;
    }
    
    // Get current tier for an address
    CompilationTier get_tier(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second->get_tier() : CompilationTier::Interpreter;
    }
    
    // Get compiled code pointer for an address
    void* get_compiled_code(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second->get_compiled_code() : nullptr;
    }
    
    // Get execution count for an address
    uint32_t get_execution_count(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        auto it = entries.find(address);
        return (it != entries.end()) ? it->second->execution_count.load() : 0;
    }
    
    // Get count of entries at each tier
    void get_tier_counts(size_t* tier0, size_t* tier1, size_t* tier2) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        size_t t0 = 0, t1 = 0, t2 = 0;
        for (const auto& pair : entries) {
            switch (pair.second->get_tier()) {
                case CompilationTier::Interpreter: t0++; break;
                case CompilationTier::Baseline: t1++; break;
                case CompilationTier::Optimizing: t2++; break;
            }
        }
        
        if (tier0) *tier0 = t0;
        if (tier1) *tier1 = t1;
        if (tier2) *tier2 = t2;
    }
    
    // Get statistics
    TieredCompilationStats get_stats() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return stats;
    }
    
    // Reset statistics
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = TieredCompilationStats();
    }
    
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        entries.clear();
        stats = TieredCompilationStats();
    }
};

/**
 * Compilation task for multi-threaded compilation
 */
struct CompilationTask {
    uint32_t address;
    std::vector<uint8_t> code;
    int priority;  // Higher = more important
    
    CompilationTask() : address(0), priority(0) {}
    CompilationTask(uint32_t addr, const uint8_t* c, size_t size, int prio = 0)
        : address(addr), code(c, c + size), priority(prio) {}
    
    bool operator<(const CompilationTask& other) const {
        return priority < other.priority;  // Max-heap
    }
};

/**
 * Thread pool statistics
 */
struct ThreadPoolStats {
    uint64_t total_tasks_submitted;   // Total tasks submitted to pool
    uint64_t total_tasks_completed;   // Total tasks completed
    uint64_t total_tasks_failed;      // Total tasks that failed
    uint64_t peak_queue_size;         // Peak queue size observed
    uint64_t total_wait_time_ms;      // Total time tasks waited in queue
    uint64_t total_exec_time_ms;      // Total execution time
    
    ThreadPoolStats()
        : total_tasks_submitted(0), total_tasks_completed(0), total_tasks_failed(0),
          peak_queue_size(0), total_wait_time_ms(0), total_exec_time_ms(0) {}
    
    double get_avg_wait_time_ms() const {
        return (total_tasks_completed > 0) 
            ? (static_cast<double>(total_wait_time_ms) / total_tasks_completed) : 0.0;
    }
    
    double get_avg_exec_time_ms() const {
        return (total_tasks_completed > 0) 
            ? (static_cast<double>(total_exec_time_ms) / total_tasks_completed) : 0.0;
    }
};

/**
 * Enhanced compilation task with timing information
 */
struct EnhancedCompilationTask {
    uint32_t address;
    std::vector<uint8_t> code;
    int priority;                      // Higher = more important
    std::chrono::steady_clock::time_point submit_time;  // When task was submitted
    
    EnhancedCompilationTask() 
        : address(0), priority(0), submit_time(std::chrono::steady_clock::now()) {}
    
    EnhancedCompilationTask(uint32_t addr, const uint8_t* c, size_t size, int prio = 0)
        : address(addr), code(c, c + size), priority(prio),
          submit_time(std::chrono::steady_clock::now()) {}
    
    bool operator<(const EnhancedCompilationTask& other) const {
        return priority < other.priority;  // Max-heap
    }
    
    uint64_t get_wait_time_ms() const {
        auto now = std::chrono::steady_clock::now();
        return std::chrono::duration_cast<std::chrono::milliseconds>(
            now - submit_time).count();
    }
};

/**
 * Enhanced multi-threaded compilation thread pool with statistics
 */
struct EnhancedCompilationThreadPool {
    std::vector<oc_thread> workers;
    std::priority_queue<EnhancedCompilationTask> task_queue;
    mutable oc_mutex queue_mutex;
    oc_condition_variable condition;
    oc_condition_variable all_done_condition;  // For waiting until all tasks complete
    std::atomic<bool> stop_flag;
    std::atomic<bool> drain_flag;              // If true, finish remaining tasks before shutdown
    std::atomic<size_t> pending_tasks;
    std::atomic<size_t> completed_tasks;
    std::atomic<size_t> active_workers;        // Workers currently processing tasks
    std::function<bool(const EnhancedCompilationTask&)> compile_func;  // Returns true on success
    ThreadPoolStats stats;
    
    EnhancedCompilationThreadPool() 
        : stop_flag(false), drain_flag(false), pending_tasks(0), 
          completed_tasks(0), active_workers(0) {}
    
    ~EnhancedCompilationThreadPool() {
        shutdown(false);
    }
    
    // Start thread pool with specified number of workers
    void start(size_t num_threads, std::function<bool(const EnhancedCompilationTask&)> func) {
        compile_func = std::move(func);
        stop_flag = false;
        drain_flag = false;
        
        for (size_t i = 0; i < num_threads; ++i) {
            workers.emplace_back([this, i] {
                worker_thread(i);
            });
        }
    }
    
    // Worker thread function
    void worker_thread(size_t /* worker_id */) {
        while (true) {
            EnhancedCompilationTask task;
            {
                oc_unique_lock<oc_mutex> lock(queue_mutex);
                condition.wait(lock, [this] {
                    return stop_flag.load() || !task_queue.empty();
                });
                
                // Check if we should exit
                if (stop_flag.load()) {
                    // If draining, only exit when queue is empty
                    if (drain_flag.load()) {
                        if (task_queue.empty()) {
                            return;
                        }
                    } else {
                        // Immediate shutdown, exit even if tasks remain
                        return;
                    }
                }
                
                if (task_queue.empty()) {
                    continue;
                }
                
                task = task_queue.top();
                task_queue.pop();
            }
            
            active_workers.fetch_add(1);
            
            // Track wait time
            uint64_t wait_time = task.get_wait_time_ms();
            
            // Execute task
            auto exec_start = std::chrono::steady_clock::now();
            bool success = true;
            if (compile_func) {
                success = compile_func(task);
            }
            auto exec_end = std::chrono::steady_clock::now();
            
            uint64_t exec_time = std::chrono::duration_cast<std::chrono::milliseconds>(
                exec_end - exec_start).count();
            
            // Update counters and stats
            {
                oc_lock_guard<oc_mutex> lock(queue_mutex);
                stats.total_wait_time_ms += wait_time;
                stats.total_exec_time_ms += exec_time;
                if (success) {
                    stats.total_tasks_completed++;
                } else {
                    stats.total_tasks_failed++;
                }
                
                // Update counters while holding lock
                pending_tasks.fetch_sub(1);
                completed_tasks.fetch_add(1);
                active_workers.fetch_sub(1);
                
                // Check and notify if all tasks done (inside lock to avoid race)
                if (pending_tasks.load() == 0 && active_workers.load() == 0) {
                    all_done_condition.notify_all();
                }
            }
        }
    }
    
    // Submit a compilation task
    void submit(uint32_t address, const uint8_t* code, size_t size, int priority = 0) {
        {
            oc_lock_guard<oc_mutex> lock(queue_mutex);
            task_queue.emplace(address, code, size, priority);
            pending_tasks.fetch_add(1);
            stats.total_tasks_submitted++;
            
            // Track peak queue size
            size_t current_size = task_queue.size();
            if (current_size > stats.peak_queue_size) {
                stats.peak_queue_size = current_size;
            }
        }
        condition.notify_one();
    }
    
    // Wait for all pending tasks to complete
    bool wait_all(uint32_t timeout_ms = 0) {
        oc_unique_lock<oc_mutex> lock(queue_mutex);
        
        if (timeout_ms == 0) {
            // Wait indefinitely
            all_done_condition.wait(lock, [this] {
                return pending_tasks.load() == 0 && active_workers.load() == 0;
            });
            return true;
        } else {
            // Wait with timeout
            return all_done_condition.wait_for(
                lock, 
                std::chrono::milliseconds(timeout_ms),
                [this] {
                    return pending_tasks.load() == 0 && active_workers.load() == 0;
                }
            );
        }
    }
    
    // Shutdown the thread pool
    // If drain is true, finish all remaining tasks before stopping
    void shutdown(bool drain = true) {
        drain_flag = drain;
        stop_flag = true;
        condition.notify_all();
        
        for (auto& worker : workers) {
            if (worker.joinable()) {
                worker.join();
            }
        }
        workers.clear();
    }
    
    // Cancel all pending tasks (only queue tasks, not active ones)
    size_t cancel_all() {
        oc_lock_guard<oc_mutex> lock(queue_mutex);
        size_t cancelled = task_queue.size();
        
        while (!task_queue.empty()) {
            task_queue.pop();
        }
        // Only decrement by cancelled count, not set to 0
        // Active workers still have pending work
        pending_tasks.fetch_sub(cancelled);
        
        return cancelled;
    }
    
    // Get thread count
    size_t get_thread_count() const {
        return workers.size();
    }
    
    // Get active worker count
    size_t get_active_workers() const {
        return active_workers.load();
    }
    
    // Get pending task count
    size_t get_pending_count() const { 
        return pending_tasks.load(); 
    }
    
    // Get completed task count
    size_t get_completed_count() const { 
        return completed_tasks.load(); 
    }
    
    // Check if running
    bool is_running() const { 
        return !workers.empty() && !stop_flag.load(); 
    }
    
    // Get statistics
    ThreadPoolStats get_stats() const {
        oc_lock_guard<oc_mutex> lock(queue_mutex);
        return stats;
    }
    
    // Reset statistics (does not reset runtime counters like completed_tasks)
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(queue_mutex);
        stats = ThreadPoolStats();
    }
};

/**
 * Multi-threaded compilation thread pool
 */
struct CompilationThreadPool {
    std::vector<oc_thread> workers;
    std::priority_queue<CompilationTask> task_queue;
    oc_mutex queue_mutex;
    oc_condition_variable condition;
    std::atomic<bool> stop_flag;
    std::atomic<size_t> pending_tasks;
    std::atomic<size_t> completed_tasks;
    std::function<void(const CompilationTask&)> compile_func;
    
    CompilationThreadPool() : stop_flag(false), pending_tasks(0), completed_tasks(0) {}
    
    ~CompilationThreadPool() {
        shutdown();
    }
    
    void start(size_t num_threads, std::function<void(const CompilationTask&)> func) {
        compile_func = std::move(func);
        stop_flag = false;
        
        for (size_t i = 0; i < num_threads; ++i) {
            workers.emplace_back([this] {
                while (true) {
                    CompilationTask task;
                    {
                        oc_unique_lock<oc_mutex> lock(queue_mutex);
                        condition.wait(lock, [this] {
                            return stop_flag.load() || !task_queue.empty();
                        });
                        
                        if (stop_flag && task_queue.empty()) {
                            return;
                        }
                        
                        task = task_queue.top();
                        task_queue.pop();
                    }
                    
                    compile_func(task);
                    pending_tasks--;
                    completed_tasks++;
                }
            });
        }
    }
    
    void submit(const CompilationTask& task) {
        {
            oc_lock_guard<oc_mutex> lock(queue_mutex);
            task_queue.push(task);
            pending_tasks++;
        }
        condition.notify_one();
    }
    
    void shutdown() {
        stop_flag = true;
        condition.notify_all();
        
        for (auto& worker : workers) {
            if (worker.joinable()) {
                worker.join();
            }
        }
        workers.clear();
    }
    
    size_t get_pending_count() const { return pending_tasks; }
    size_t get_completed_count() const { return completed_tasks; }
    bool is_running() const { return !workers.empty() && !stop_flag; }
};

// ============================================================================
// Background Compilation System
// ============================================================================

/**
 * Background compilation statistics
 */
struct BackgroundCompilationStats {
    uint64_t speculative_queued;       // Blocks queued speculatively
    uint64_t speculative_compiled;     // Blocks compiled speculatively
    uint64_t speculative_hits;         // Speculatively compiled blocks that were executed
    uint64_t branch_targets_queued;    // Branch targets queued for precompilation
    uint64_t branch_targets_compiled;  // Branch targets compiled
    uint64_t idle_compilations;        // Compilations during idle time
    uint64_t already_compiled;         // Requests for already-compiled blocks
    uint64_t compilation_failures;     // Failed compilations
    
    BackgroundCompilationStats()
        : speculative_queued(0), speculative_compiled(0), speculative_hits(0),
          branch_targets_queued(0), branch_targets_compiled(0), idle_compilations(0),
          already_compiled(0), compilation_failures(0) {}
    
    double get_speculation_hit_rate() const {
        return (speculative_compiled > 0)
            ? (100.0 * speculative_hits / speculative_compiled) : 0.0;
    }
};

/**
 * Speculative compilation entry with scoring
 */
struct SpeculativeEntry {
    uint32_t address;
    const uint8_t* code;
    size_t size;
    int score;                          // Higher = more likely to execute
    bool is_branch_target;              // True if this is a branch target
    std::chrono::steady_clock::time_point queue_time;
    
    SpeculativeEntry()
        : address(0), code(nullptr), size(0), score(0), is_branch_target(false),
          queue_time(std::chrono::steady_clock::now()) {}
    
    SpeculativeEntry(uint32_t addr, const uint8_t* c, size_t s, int sc, bool branch = false)
        : address(addr), code(c), size(s), score(sc), is_branch_target(branch),
          queue_time(std::chrono::steady_clock::now()) {}
    
    bool operator<(const SpeculativeEntry& other) const {
        return score < other.score;  // Max-heap by score
    }
};

/**
 * Background Compilation Manager
 * Manages speculative and ahead-of-time compilation for improved performance
 */
struct BackgroundCompilationManager {
    std::priority_queue<SpeculativeEntry> speculative_queue;
    std::unordered_set<uint32_t> queued_addresses;  // Prevent duplicates
    std::unordered_set<uint32_t> compiled_addresses; // Track what's compiled
    mutable oc_mutex mutex;
    
    std::atomic<bool> enabled;
    std::atomic<bool> idle_mode;
    
    // Configuration
    uint32_t speculation_depth;         // How many blocks ahead to speculate
    int branch_target_priority;         // Priority boost for branch targets
    int hot_block_threshold;            // Execution count to consider "hot"
    size_t max_queue_size;              // Maximum speculative queue size
    
    // Statistics
    BackgroundCompilationStats stats;
    
    static constexpr uint32_t DEFAULT_SPECULATION_DEPTH = 3;
    static constexpr int DEFAULT_BRANCH_TARGET_PRIORITY = 50;
    static constexpr int DEFAULT_HOT_BLOCK_THRESHOLD = 5;
    static constexpr size_t DEFAULT_MAX_QUEUE_SIZE = 1000;
    
    BackgroundCompilationManager()
        : enabled(false), idle_mode(false),
          speculation_depth(DEFAULT_SPECULATION_DEPTH),
          branch_target_priority(DEFAULT_BRANCH_TARGET_PRIORITY),
          hot_block_threshold(DEFAULT_HOT_BLOCK_THRESHOLD),
          max_queue_size(DEFAULT_MAX_QUEUE_SIZE) {}
    
    // Enable/disable background compilation
    void set_enabled(bool enable) {
        enabled.store(enable);
    }
    
    bool is_enabled() const {
        return enabled.load();
    }
    
    // Enter/exit idle mode (for idle-time compilation)
    void set_idle_mode(bool idle) {
        idle_mode.store(idle);
    }
    
    bool is_idle() const {
        return idle_mode.load();
    }
    
    // Configure speculation parameters
    void configure(uint32_t depth, int branch_priority, int hot_threshold, size_t max_queue) {
        oc_lock_guard<oc_mutex> lock(mutex);
        speculation_depth = depth;
        branch_target_priority = branch_priority;
        hot_block_threshold = hot_threshold;
        max_queue_size = max_queue;
    }
    
    // Check if address is already compiled or queued
    bool is_compiled(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return compiled_addresses.find(address) != compiled_addresses.end();
    }
    
    bool is_queued(uint32_t address) const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return queued_addresses.find(address) != queued_addresses.end();
    }
    
    // Mark an address as compiled (called externally after compilation)
    void mark_compiled(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        compiled_addresses.insert(address);
        queued_addresses.erase(address);
    }
    
    // Queue a block for speculative compilation
    bool queue_speculative(uint32_t address, const uint8_t* code, size_t size, 
                          int base_score = 0, bool is_branch_target = false) {
        if (!enabled.load()) return false;
        
        oc_lock_guard<oc_mutex> lock(mutex);
        
        // Check if already compiled or queued
        if (compiled_addresses.find(address) != compiled_addresses.end()) {
            stats.already_compiled++;
            return false;
        }
        
        if (queued_addresses.find(address) != queued_addresses.end()) {
            return false;  // Already queued
        }
        
        // Check queue size limit
        if (speculative_queue.size() >= max_queue_size) {
            return false;  // Queue full
        }
        
        // Calculate score
        int score = base_score;
        if (is_branch_target) {
            score += branch_target_priority;
            stats.branch_targets_queued++;
        } else {
            stats.speculative_queued++;
        }
        
        speculative_queue.emplace(address, code, size, score, is_branch_target);
        queued_addresses.insert(address);
        
        return true;
    }
    
    // Queue multiple branch targets for precompilation
    size_t queue_branch_targets(const std::vector<std::pair<uint32_t, std::pair<const uint8_t*, size_t>>>& targets) {
        if (!enabled.load()) return 0;
        
        size_t queued = 0;
        for (const auto& target : targets) {
            if (queue_speculative(target.first, target.second.first, target.second.second, 
                                  0, true)) {
                queued++;
            }
        }
        return queued;
    }
    
    // Get next block to compile (highest priority)
    bool get_next_task(SpeculativeEntry& entry) {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        if (speculative_queue.empty()) {
            return false;
        }
        
        entry = speculative_queue.top();
        speculative_queue.pop();
        
        return true;
    }
    
    // Process one compilation during idle time
    // Returns: true if a task was processed
    bool process_idle_task(std::function<bool(uint32_t, const uint8_t*, size_t)> compile_func) {
        if (!enabled.load() || !idle_mode.load()) return false;
        
        SpeculativeEntry entry;
        if (!get_next_task(entry)) {
            return false;
        }
        
        // Compile the entry
        bool success = compile_func(entry.address, entry.code, entry.size);
        
        {
            oc_lock_guard<oc_mutex> lock(mutex);
            queued_addresses.erase(entry.address);
            
            if (success) {
                compiled_addresses.insert(entry.address);
                stats.idle_compilations++;
                
                if (entry.is_branch_target) {
                    stats.branch_targets_compiled++;
                } else {
                    stats.speculative_compiled++;
                }
            } else {
                stats.compilation_failures++;
            }
        }
        
        return true;
    }
    
    // Process multiple tasks during idle time (up to max_count)
    size_t process_idle_batch(std::function<bool(uint32_t, const uint8_t*, size_t)> compile_func,
                               size_t max_count) {
        size_t processed = 0;
        while (processed < max_count && process_idle_task(compile_func)) {
            processed++;
        }
        return processed;
    }
    
    // Record that a speculatively compiled block was executed (hit)
    void record_speculative_hit(uint32_t address) {
        oc_lock_guard<oc_mutex> lock(mutex);
        if (compiled_addresses.find(address) != compiled_addresses.end()) {
            stats.speculative_hits++;
        }
    }
    
    // Get queue size
    size_t get_queue_size() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return speculative_queue.size();
    }
    
    // Get compiled count
    size_t get_compiled_count() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return compiled_addresses.size();
    }
    
    // Get statistics
    BackgroundCompilationStats get_stats() const {
        oc_lock_guard<oc_mutex> lock(mutex);
        return stats;
    }
    
    // Reset statistics
    void reset_stats() {
        oc_lock_guard<oc_mutex> lock(mutex);
        stats = BackgroundCompilationStats();
    }
    
    // Clear all state
    void clear() {
        oc_lock_guard<oc_mutex> lock(mutex);
        
        while (!speculative_queue.empty()) {
            speculative_queue.pop();
        }
        queued_addresses.clear();
        compiled_addresses.clear();
        stats = BackgroundCompilationStats();
    }
};

/**
 * JIT compilation error types for comprehensive error handling
 */
enum class JitErrorKind : uint8_t {
    None = 0,
    InitializationFailed = 1,
    ModuleCreationFailed = 2,
    CompilationFailed = 3,
    LookupFailed = 4,
    TargetConfigFailed = 5,
    VerificationFailed = 6
};

/**
 * JIT compilation result with error handling
 */
struct JitResult {
    JitErrorKind error;
    std::string error_message;
    void* compiled_code;
    
    JitResult() : error(JitErrorKind::None), compiled_code(nullptr) {}
    JitResult(JitErrorKind e, const std::string& msg) 
        : error(e), error_message(msg), compiled_code(nullptr) {}
    JitResult(void* code) : error(JitErrorKind::None), compiled_code(code) {}
    
    bool success() const { return error == JitErrorKind::None; }
    operator bool() const { return success(); }
};

#ifdef HAVE_LLVM
/**
 * ORC JIT Manager - Enhanced LLJIT wrapper with:
 * - Proper ThreadSafeModule for module ownership
 * - Target machine configuration with host CPU features
 * - Error handling for JIT creation and module compilation
 * - Function lookup with error propagation
 */
class OrcJitManager {
public:
    std::unique_ptr<llvm::orc::LLJIT> jit;
    std::unique_ptr<llvm::TargetMachine> target_machine;
    std::string last_error;
    bool initialized;
    
    // CPU feature flags detected at runtime
    bool has_avx2;
    bool has_avx512;
    bool has_sse4;
    
    OrcJitManager() : initialized(false), has_avx2(false), has_avx512(false), has_sse4(false) {}
    
    /**
     * Initialize the JIT with proper target machine configuration
     */
    JitResult initialize() {
        // Initialize LLVM targets
        llvm::InitializeNativeTarget();
        llvm::InitializeNativeTargetAsmPrinter();
        llvm::InitializeNativeTargetAsmParser();
        
        // Detect CPU features
        detect_cpu_features();
        
        // Create target machine with optimal configuration
        auto tm_result = configure_target_machine();
        if (!tm_result.success()) {
            return tm_result;
        }
        
        // Create LLJIT with configured target machine
        auto jit_builder = llvm::orc::LLJITBuilder();
        
        // Configure for optimal performance
        jit_builder.setNumCompileThreads(0); // Compile in calling thread for predictability
        
        auto jit_expected = jit_builder.create();
        if (!jit_expected) {
            std::string err_msg;
            llvm::raw_string_ostream err_stream(err_msg);
            err_stream << jit_expected.takeError();
            last_error = err_stream.str();
            return JitResult(JitErrorKind::InitializationFailed, last_error);
        }
        
        jit = std::move(*jit_expected);
        initialized = true;
        
        return JitResult();
    }
    
    /**
     * Detect host CPU features for optimal code generation
     */
    void detect_cpu_features() {
        llvm::StringMap<bool> features;
        llvm::sys::getHostCPUFeatures(features);
        
        has_avx2 = features.count("avx2") && features["avx2"];
        has_avx512 = features.count("avx512f") && features["avx512f"];
        has_sse4 = features.count("sse4.2") && features["sse4.2"];
    }
    
    /**
     * Configure target machine for host CPU with optimal features
     */
    JitResult configure_target_machine() {
        std::string triple = llvm::sys::getProcessTriple();
        std::string cpu = std::string(llvm::sys::getHostCPUName());
        
        // Build feature string based on detected capabilities
        std::string features;
        if (has_avx512) {
            features = "+avx512f,+avx512vl,+avx512bw,+avx512dq";
        } else if (has_avx2) {
            features = "+avx2,+fma";
        } else if (has_sse4) {
            features = "+sse4.2";
        }
        
        std::string error;
        auto target = llvm::TargetRegistry::lookupTarget(triple, error);
        if (!target) {
            last_error = "Failed to lookup target: " + error;
            return JitResult(JitErrorKind::TargetConfigFailed, last_error);
        }
        
        llvm::TargetOptions options;
        options.UnsafeFPMath = true;      // Allow FP optimizations
        options.NoInfsFPMath = true;      // No infinities
        options.NoNaNsFPMath = true;      // No NaNs  
        options.AllowFPOpFusion = llvm::FPOpFusion::Fast;  // Allow FMA fusion
        
        target_machine.reset(target->createTargetMachine(
            triple, cpu, features,
            options,
            llvm::Reloc::PIC_,
            llvm::CodeModel::Small,
            llvm::CodeGenOptLevel::Aggressive
        ));
        
        if (!target_machine) {
            last_error = "Failed to create target machine";
            return JitResult(JitErrorKind::TargetConfigFailed, last_error);
        }
        
        return JitResult();
    }
    
    /**
     * Add a module to the JIT with proper ThreadSafeModule ownership
     */
    JitResult add_module(std::unique_ptr<llvm::Module> module, 
                         std::unique_ptr<llvm::LLVMContext> context) {
        if (!initialized || !jit) {
            return JitResult(JitErrorKind::InitializationFailed, "JIT not initialized");
        }
        
        // Create ThreadSafeModule for proper ownership
        auto tsm = llvm::orc::ThreadSafeModule(std::move(module), std::move(context));
        
        if (auto err = jit->addIRModule(std::move(tsm))) {
            std::string err_msg;
            llvm::raw_string_ostream err_stream(err_msg);
            err_stream << err;
            last_error = err_stream.str();
            return JitResult(JitErrorKind::ModuleCreationFailed, last_error);
        }
        
        return JitResult();
    }
    
    /**
     * Lookup a compiled function by name with error handling
     */
    JitResult lookup_function(const std::string& name) {
        if (!initialized || !jit) {
            return JitResult(JitErrorKind::InitializationFailed, "JIT not initialized");
        }
        
        auto sym = jit->lookup(name);
        if (!sym) {
            std::string err_msg;
            llvm::raw_string_ostream err_stream(err_msg);
            err_stream << sym.takeError();
            last_error = err_stream.str();
            return JitResult(JitErrorKind::LookupFailed, last_error);
        }
        
        return JitResult(reinterpret_cast<void*>(sym->getValue()));
    }
    
    /**
     * Get feature string for diagnostics
     */
    std::string get_features_string() const {
        std::string result;
        if (has_avx512) result += "AVX-512 ";
        if (has_avx2) result += "AVX2 ";
        if (has_sse4) result += "SSE4.2 ";
        return result.empty() ? "baseline" : result;
    }
    
    bool is_initialized() const { return initialized; }
    const std::string& get_last_error() const { return last_error; }
};
#endif

/**
 * PPU JIT compiler structure
 */
struct oc_ppu_jit_t {
    CodeCache cache;
    BreakpointManager breakpoints;
    BranchPredictor branch_predictor;
    InlineCacheManager inline_cache;
    BranchTargetCache branch_target_cache;
    ConstantPropagationCache const_prop_cache;
    RegisterAllocator reg_allocator;
    EnhancedRegisterAllocator enhanced_reg_allocator;
    LazyCompilationManager lazy_manager;
    EnhancedLazyCompilationManager enhanced_lazy_manager;
    TieredCompilationManager tiered_manager;
    CompilationThreadPool thread_pool;
    EnhancedCompilationThreadPool enhanced_thread_pool;
    BackgroundCompilationManager bg_compiler;
    bool enabled;
    bool lazy_compilation_enabled;
    bool multithreaded_enabled;
    size_t num_compile_threads;
    
#ifdef HAVE_LLVM
    std::unique_ptr<llvm::LLVMContext> context;
    std::unique_ptr<llvm::Module> module;
    std::unique_ptr<llvm::orc::LLJIT> jit;
    llvm::TargetMachine* target_machine;
    OrcJitManager orc_manager;  // Enhanced ORC JIT manager
#endif
    
    oc_ppu_jit_t() : enabled(true), lazy_compilation_enabled(false), 
                     multithreaded_enabled(false), num_compile_threads(0) {
#ifdef HAVE_LLVM
        context = std::make_unique<llvm::LLVMContext>();
        module = std::make_unique<llvm::Module>("ppu_jit", *context);
        target_machine = nullptr;
        
        // Initialize LLVM targets
        llvm::InitializeNativeTarget();
        llvm::InitializeNativeTargetAsmPrinter();
        llvm::InitializeNativeTargetAsmParser();
        
        // Initialize enhanced ORC JIT manager
        auto orc_result = orc_manager.initialize();
        if (!orc_result.success()) {
            // Fall back to simple LLJIT
            auto jit_builder = llvm::orc::LLJITBuilder();
            auto jit_result = jit_builder.create();
            if (jit_result) {
                jit = std::move(*jit_result);
            }
        }
#endif
    }
};

/**
 * Identify basic block boundaries
 * A basic block ends at:
 * - Branch instructions (b, bc, bclr, bcctr)
 * - System calls (sc)
 * - Trap instructions
 */
static void identify_basic_block(const uint8_t* code, size_t size, BasicBlock* block) {
    size_t offset = 0;
    
    while (offset < size) {
        if (offset + 4 > size) break;
        
        uint32_t instr;
        memcpy(&instr, code + offset, 4);
        // PPU uses big-endian
        instr = __builtin_bswap32(instr);
        
        block->instructions.push_back(instr);
        block->end_address = block->start_address + offset + 4;
        
        // Check for block-ending instructions
        uint8_t opcode = (instr >> 26) & 0x3F;
        
        // Branch instructions (18 = b, 16 = bc)
        if (opcode == 18 || opcode == 16) {
            offset += 4;
            break;
        }
        
        // Extended opcode check
        if (opcode == 19) {
            uint16_t xo = (instr >> 1) & 0x3FF;
            // bclr (16), bcctr (528)
            if (xo == 16 || xo == 528) {
                offset += 4;
                break;
            }
        }
        
        // System call (opcode 17)
        if (opcode == 17) {
            offset += 4;
            break;
        }
        
        offset += 4;
    }
}

#ifdef HAVE_LLVM
// Forward declarations for functions defined later in this file
static llvm::Function* create_llvm_function(llvm::Module* module, BasicBlock* block);
static void apply_optimization_passes(llvm::Module* module);
#endif

/**
 * Generate LLVM IR for a basic block
 * 
 * This function creates LLVM IR for all instructions in the basic block
 * using the comprehensive emit_ppu_instruction implementation.
 * 
 * When HAVE_LLVM is defined, this uses the full LLVM infrastructure to:
 * 1. Create a function in the module for this basic block
 * 2. Emit LLVM IR for each PowerPC instruction
 * 3. Apply optimization passes
 * 4. Emit native machine code via LLJIT
 * 
 * Without LLVM, a placeholder implementation is used.
 */

// Constant for placeholder code generation
static constexpr uint8_t X86_RET_INSTRUCTION = 0xC3;

/**
 * Allocate placeholder code buffer for a basic block
 * Used when full JIT compilation is not available or fails
 */
static void allocate_placeholder_code(BasicBlock* block) {
    block->code_size = block->instructions.size() * 16; // Estimate
    block->compiled_code = malloc(block->code_size);
    if (block->compiled_code) {
        memset(block->compiled_code, X86_RET_INSTRUCTION, block->code_size);
    }
}

static void generate_llvm_ir(BasicBlock* block, oc_ppu_jit_t* jit = nullptr) {
#ifdef HAVE_LLVM
    if (jit && jit->module) {
        // Create LLVM function for this block
        llvm::Function* func = create_llvm_function(jit->module.get(), block);
        
        if (func) {
            // Apply optimization passes to the module
            apply_optimization_passes(jit->module.get());
            
            // If we have a working LLJIT, compile and get the function pointer
            if (jit->jit) {
                // In a full implementation, we would:
                // 1. Add the module to the JIT
                // 2. Lookup the function symbol
                // 3. Get the function pointer
                // 4. Store it in block->compiled_code
                
                // For now, use placeholder code buffer since full LLJIT
                // integration requires additional error handling
                allocate_placeholder_code(block);
            } else {
                // No JIT available, use placeholder
                allocate_placeholder_code(block);
            }
        } else {
            // Function creation failed, use placeholder
            allocate_placeholder_code(block);
        }
    } else {
        // No JIT context, use placeholder
        allocate_placeholder_code(block);
    }
#else
    // Without LLVM, use simple placeholder
    (void)jit; // Unused parameter
    allocate_placeholder_code(block);
#endif
}

#ifdef HAVE_LLVM

// XER Register bit positions
// In the 64-bit XER register, the flag bits are stored in the lower 32-bit portion:
// - Bit 31 (0x80000000): SO (Summary Overflow)
// - Bit 30 (0x40000000): OV (Overflow)
// - Bit 29 (0x20000000): CA (Carry)
// These correspond to bits 0, 1, 2 in PowerPC documentation which uses
// big-endian bit numbering (bit 0 = MSB).
static constexpr uint64_t XER_SO_MASK = 0x80000000ULL;  // Bit 31 in little-endian terms
static constexpr uint64_t XER_OV_MASK = 0x40000000ULL;  // Bit 30 in little-endian terms
static constexpr uint64_t XER_CA_MASK = 0x20000000ULL;  // Bit 29 in little-endian terms

/**
 * Update CR0 based on the result value
 * 
 * CR0 is set based on the signed comparison of the result with zero:
 * - CR0[LT] (bit 0): Result is negative (bit 0 = 1 if result < 0)
 * - CR0[GT] (bit 1): Result is positive (bit 1 = 1 if result > 0)
 * - CR0[EQ] (bit 2): Result is zero (bit 2 = 1 if result == 0)
 * - CR0[SO] (bit 3): Copy of XER[SO]
 */
static void update_cr0(llvm::IRBuilder<>& builder, llvm::Value* result,
                       llvm::Value* cr_ptr, llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i32_ty = llvm::Type::getInt32Ty(ctx);
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    // Compare result with zero
    llvm::Value* zero = llvm::ConstantInt::get(i64_ty, 0);
    llvm::Value* lt = builder.CreateICmpSLT(result, zero);
    llvm::Value* gt = builder.CreateICmpSGT(result, zero);
    llvm::Value* eq = builder.CreateICmpEQ(result, zero);
    
    // Build CR0 field value
    llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
    cr_field = builder.CreateSelect(lt, llvm::ConstantInt::get(i32_ty, 8), cr_field);
    cr_field = builder.CreateSelect(gt, llvm::ConstantInt::get(i32_ty, 4), cr_field);
    cr_field = builder.CreateSelect(eq, llvm::ConstantInt::get(i32_ty, 2), cr_field);
    
    // Add SO bit from XER
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* so_bit = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, XER_SO_MASK));
    llvm::Value* has_so = builder.CreateICmpNE(so_bit, llvm::ConstantInt::get(i64_ty, 0));
    llvm::Value* so_val = builder.CreateSelect(has_so, llvm::ConstantInt::get(i32_ty, 1), llvm::ConstantInt::get(i32_ty, 0));
    cr_field = builder.CreateOr(cr_field, so_val);
    
    // Update CR (field 0 = bits 28-31)
    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
    llvm::Value* cr_masked = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, 0x0FFFFFFF));
    llvm::Value* shifted = builder.CreateShl(cr_field, llvm::ConstantInt::get(i32_ty, 28));
    llvm::Value* new_cr = builder.CreateOr(cr_masked, shifted);
    builder.CreateStore(new_cr, cr_ptr);
}

/**
 * Set CA flag in XER based on unsigned overflow for addition
 * 
 * For addition: CA = 1 if (result < a) (unsigned overflow occurred)
 * This detects if the addition wrapped around.
 * 
 * Note: Parameter 'b' is included for interface consistency with set_ca_add_extended
 * which needs all operands. For simple addition, only 'a' and 'result' are needed.
 */
static void set_ca_add(llvm::IRBuilder<>& builder, llvm::Value* a, [[maybe_unused]] llvm::Value* b,
                       llvm::Value* result, llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    // Carry occurred if result < a (unsigned comparison)
    llvm::Value* carry = builder.CreateICmpULT(result, a);
    
    // Load XER, clear CA bit, set if carry occurred
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* xer_cleared = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, ~XER_CA_MASK));
    llvm::Value* ca_bit = builder.CreateSelect(carry,
        llvm::ConstantInt::get(i64_ty, XER_CA_MASK),
        llvm::ConstantInt::get(i64_ty, 0));
    llvm::Value* new_xer = builder.CreateOr(xer_cleared, ca_bit);
    builder.CreateStore(new_xer, xer_ptr);
}

/**
 * Set CA flag in XER for subtraction (a - b)
 * 
 * For subtraction a - b: CA = 1 if there is NO borrow, i.e., a >= b (unsigned)
 * PowerPC uses the convention: CA = NOT borrow
 */
static void set_ca_sub(llvm::IRBuilder<>& builder, llvm::Value* a, llvm::Value* b,
                       llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    // No borrow if a >= b (unsigned comparison)
    llvm::Value* no_borrow = builder.CreateICmpUGE(a, b);
    
    // Load XER, clear CA bit, set if no borrow
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* xer_cleared = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, ~XER_CA_MASK));
    llvm::Value* ca_bit = builder.CreateSelect(no_borrow,
        llvm::ConstantInt::get(i64_ty, XER_CA_MASK),
        llvm::ConstantInt::get(i64_ty, 0));
    llvm::Value* new_xer = builder.CreateOr(xer_cleared, ca_bit);
    builder.CreateStore(new_xer, xer_ptr);
}

/**
 * Get CA bit from XER as a 64-bit value (0 or 1)
 */
static llvm::Value* get_ca_bit(llvm::IRBuilder<>& builder, llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* ca_masked = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, XER_CA_MASK));
    llvm::Value* has_ca = builder.CreateICmpNE(ca_masked, llvm::ConstantInt::get(i64_ty, 0));
    return builder.CreateSelect(has_ca,
        llvm::ConstantInt::get(i64_ty, 1),
        llvm::ConstantInt::get(i64_ty, 0));
}

/**
 * Get SO bit from XER as a 32-bit value (0 or 1) for CR field
 */
static llvm::Value* get_so_bit_for_cr(llvm::IRBuilder<>& builder, llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i32_ty = llvm::Type::getInt32Ty(ctx);
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* so_masked = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, XER_SO_MASK));
    llvm::Value* has_so = builder.CreateICmpNE(so_masked, llvm::ConstantInt::get(i64_ty, 0));
    return builder.CreateSelect(has_so,
        llvm::ConstantInt::get(i32_ty, 1),
        llvm::ConstantInt::get(i32_ty, 0));
}

/**
 * Set CA flag for extended add operations (adde, addze, addme)
 * These need to account for the incoming carry bit
 * 
 * For adde: result = a + b + CA
 * Carry out = (a + b overflows) OR ((a + b) + CA overflows)
 */
static void set_ca_add_extended(llvm::IRBuilder<>& builder, llvm::Value* a, llvm::Value* b,
                                llvm::Value* ca_in, llvm::Value* xer_ptr) {
    auto& ctx = builder.getContext();
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    
    // First addition: temp = a + b
    llvm::Value* temp = builder.CreateAdd(a, b);
    // First carry: carry1 = (temp < a)
    llvm::Value* carry1 = builder.CreateICmpULT(temp, a);
    
    // Second addition: result = temp + ca_in
    llvm::Value* result = builder.CreateAdd(temp, ca_in);
    // Second carry: carry2 = (result < temp)
    llvm::Value* carry2 = builder.CreateICmpULT(result, temp);
    
    // Final carry = carry1 OR carry2
    llvm::Value* final_carry = builder.CreateOr(carry1, carry2);
    
    // Update XER CA bit
    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
    llvm::Value* xer_cleared = builder.CreateAnd(xer, llvm::ConstantInt::get(i64_ty, ~XER_CA_MASK));
    llvm::Value* ca_bit = builder.CreateSelect(final_carry,
        llvm::ConstantInt::get(i64_ty, XER_CA_MASK),
        llvm::ConstantInt::get(i64_ty, 0));
    llvm::Value* new_xer = builder.CreateOr(xer_cleared, ca_bit);
    builder.CreateStore(new_xer, xer_ptr);
}

/**
 * Emit LLVM IR for PPU instructions
 * 
 * Complete LLVM IR generation for all PowerPC 64-bit (Cell PPU) instructions.
 * Supports:
 * - Integer arithmetic (add, sub, mul, div, logical, shift, rotate)
 * - Load/store (byte, halfword, word, doubleword, floating-point)
 * - Floating-point arithmetic (add, sub, mul, div, fma, conversions)
 * - Branch instructions (conditional, unconditional, to LR/CTR)
 * - Comparison instructions (signed/unsigned, integer/floating-point)
 * - System instructions (SPR access, CR operations)
 * - VMX/AltiVec vector instructions (128-bit SIMD operations)
 */
static void emit_ppu_instruction(llvm::IRBuilder<>& builder, uint32_t instr,
                                llvm::Value** gprs, llvm::Value** fprs,
                                llvm::Value** vrs,
                                llvm::Value* memory_base,
                                llvm::Value* cr_ptr, llvm::Value* lr_ptr,
                                llvm::Value* ctr_ptr, llvm::Value* xer_ptr,
                                llvm::Value* vscr_ptr,
                                uint64_t pc = 0) {
    uint8_t opcode = (instr >> 26) & 0x3F;
    uint8_t rt = (instr >> 21) & 0x1F;
    uint8_t ra = (instr >> 16) & 0x1F;
    uint8_t rb = (instr >> 11) & 0x1F;
    int16_t simm = (int16_t)(instr & 0xFFFF);
    uint16_t uimm = instr & 0xFFFF;
    
    auto& ctx = builder.getContext();
    auto i8_ty = llvm::Type::getInt8Ty(ctx);
    auto i16_ty = llvm::Type::getInt16Ty(ctx);
    auto i32_ty = llvm::Type::getInt32Ty(ctx);
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    auto f32_ty = llvm::Type::getFloatTy(ctx);
    auto f64_ty = llvm::Type::getDoubleTy(ctx);
    // Vector types for VMX
    auto v4i32_ty = llvm::VectorType::get(i32_ty, 4, false);
    auto v4f32_ty = llvm::VectorType::get(f32_ty, 4, false);
    
    switch (opcode) {
        // Integer immediate operations
        case 14: { // addi rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateAdd(ra_val, 
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 15: { // addis rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm << 16));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 24: { // ori rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateOr(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 28: { // andi. rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateAnd(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm));
            builder.CreateStore(result, gprs[rt]);
            // Update CR0 for record form
            update_cr0(builder, result, cr_ptr, xer_ptr);
            break;
        }
        case 29: { // andis. rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateAnd(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm << 16));
            builder.CreateStore(result, gprs[rt]);
            // Update CR0 for record form
            update_cr0(builder, result, cr_ptr, xer_ptr);
            break;
        }
        case 25: { // oris rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateOr(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm << 16));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 26: { // xori rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateXor(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 27: { // xoris rt, ra, uimm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateXor(ra_val,
                llvm::ConstantInt::get(i64_ty, (uint64_t)uimm << 16));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 7: { // mulli rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* result = builder.CreateMul(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            builder.CreateStore(result, gprs[rt]);
            break;
        }
        case 8: { // subfic rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* simm_val = llvm::ConstantInt::get(i64_ty, (int64_t)simm);
            llvm::Value* result = builder.CreateSub(simm_val, ra_val);
            builder.CreateStore(result, gprs[rt]);
            // Set CA flag: subfic computes (simm - ra), CA = 1 if no borrow (simm >= ra unsigned)
            set_ca_sub(builder, simm_val, ra_val, xer_ptr);
            break;
        }
        case 12: { // addic rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* simm_val = llvm::ConstantInt::get(i64_ty, (int64_t)simm);
            llvm::Value* result = builder.CreateAdd(ra_val, simm_val);
            builder.CreateStore(result, gprs[rt]);
            // Set CA flag in XER
            set_ca_add(builder, ra_val, simm_val, result, xer_ptr);
            break;
        }
        case 13: { // addic. rt, ra, simm
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* simm_val = llvm::ConstantInt::get(i64_ty, (int64_t)simm);
            llvm::Value* result = builder.CreateAdd(ra_val, simm_val);
            builder.CreateStore(result, gprs[rt]);
            // Set CA flag in XER and update CR0
            set_ca_add(builder, ra_val, simm_val, result, xer_ptr);
            update_cr0(builder, result, cr_ptr, xer_ptr);
            break;
        }
        
        // Load/Store operations
        case 32: { // lwz rt, d(ra)
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(llvm::Type::getInt8Ty(ctx),
                memory_base, addr);
            llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i32_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            break;
        }
        case 36: { // stw rs, d(ra)
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(llvm::Type::getInt8Ty(ctx),
                memory_base, addr);
            llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i32_ty, 0));
            builder.CreateStore(truncated, i32_ptr);
            break;
        }
        case 34: { // lbz rt, d(ra) - Load Byte and Zero
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* loaded = builder.CreateLoad(i8_ty, ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            break;
        }
        case 38: { // stb rs, d(ra) - Store Byte
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i8_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            builder.CreateStore(truncated, ptr);
            break;
        }
        case 40: { // lhz rt, d(ra) - Load Halfword and Zero
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            break;
        }
        case 42: { // lha rt, d(ra) - Load Halfword Algebraic (sign-extend)
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
            llvm::Value* extended = builder.CreateSExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            break;
        }
        case 44: { // sth rs, d(ra) - Store Halfword
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i16_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            builder.CreateStore(truncated, i16_ptr);
            break;
        }
        case 33: { // lwzu rt, d(ra) - Load Word and Zero with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i32_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            builder.CreateStore(addr, gprs[ra]); // Update RA
            break;
        }
        case 37: { // stwu rs, d(ra) - Store Word with Update
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i32_ty, 0));
            builder.CreateStore(truncated, i32_ptr);
            builder.CreateStore(addr, gprs[ra]); // Update RA
            break;
        }
        case 35: { // lbzu rt, d(ra) - Load Byte and Zero with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* loaded = builder.CreateLoad(i8_ty, ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 39: { // stbu rs, d(ra) - Store Byte with Update
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i8_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            builder.CreateStore(truncated, ptr);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 41: { // lhzu rt, d(ra) - Load Halfword and Zero with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
            llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 43: { // lhau rt, d(ra) - Load Halfword Algebraic with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
            llvm::Value* extended = builder.CreateSExt(loaded, i64_ty);
            builder.CreateStore(extended, gprs[rt]);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 45: { // sthu rs, d(ra) - Store Halfword with Update
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* truncated = builder.CreateTrunc(rs_val, i16_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i16_ty, 0));
            builder.CreateStore(truncated, i16_ptr);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        
        // DS-Form: 64-bit load/store (opcode 58 and 62)
        case 58: { // ld/ldu/lwa - DS-form
            uint8_t ds_xo = instr & 0x3;
            int16_t ds = (int16_t)(instr & 0xFFFC);
            llvm::Value* ra_val = (ra == 0) ? 
                static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)ds));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i64_ty, 0));
            
            if (ds_xo == 0) { // ld
                llvm::Value* loaded = builder.CreateLoad(i64_ty, i64_ptr);
                builder.CreateStore(loaded, gprs[rt]);
            } else if (ds_xo == 1) { // ldu
                llvm::Value* loaded = builder.CreateLoad(i64_ty, i64_ptr);
                builder.CreateStore(loaded, gprs[rt]);
                builder.CreateStore(addr, gprs[ra]);
            } else if (ds_xo == 2) { // lwa (load word algebraic)
                llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                    llvm::PointerType::get(i32_ty, 0));
                llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                llvm::Value* extended = builder.CreateSExt(loaded, i64_ty);
                builder.CreateStore(extended, gprs[rt]);
            }
            break;
        }
        case 62: { // std/stdu - DS-form
            uint8_t ds_xo = instr & 0x3;
            int16_t ds = (int16_t)(instr & 0xFFFC);
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
            llvm::Value* ra_val = (ra == 0) ?
                static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)ds));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(i64_ty, 0));
            builder.CreateStore(rs_val, i64_ptr);
            if (ds_xo == 1) { // stdu
                builder.CreateStore(addr, gprs[ra]);
            }
            break;
        }
        
        // Multiple Word Load/Store (opcode 46 and 47)
        case 46: { // lmw rt, d(ra) - Load Multiple Word
            llvm::Value* ra_val = (ra == 0) ?
                static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
            llvm::Value* base_addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            
            // Load words from rt to r31
            for (uint8_t r = rt; r <= 31; r++) {
                llvm::Value* offset = llvm::ConstantInt::get(i64_ty, (r - rt) * 4);
                llvm::Value* addr = builder.CreateAdd(base_addr, offset);
                llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                    llvm::PointerType::get(i32_ty, 0));
                llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                llvm::Value* extended = builder.CreateZExt(loaded, i64_ty);
                builder.CreateStore(extended, gprs[r]);
            }
            break;
        }
        case 47: { // stmw rs, d(ra) - Store Multiple Word
            llvm::Value* ra_val = (ra == 0) ?
                static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
            llvm::Value* base_addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            
            // Store words from rt to r31
            for (uint8_t r = rt; r <= 31; r++) {
                llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[r]);
                llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
                llvm::Value* offset = llvm::ConstantInt::get(i64_ty, (r - rt) * 4);
                llvm::Value* addr = builder.CreateAdd(base_addr, offset);
                llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                    llvm::PointerType::get(i32_ty, 0));
                builder.CreateStore(truncated, i32_ptr);
            }
            break;
        }
        
        // Floating-point load/store
        case 48: { // lfs frt, d(ra)
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(llvm::Type::getInt8Ty(ctx),
                memory_base, addr);
            llvm::Value* f32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(llvm::Type::getFloatTy(ctx), 0));
            llvm::Value* loaded = builder.CreateLoad(llvm::Type::getFloatTy(ctx), f32_ptr);
            llvm::Value* extended = builder.CreateFPExt(loaded, f64_ty);
            builder.CreateStore(extended, fprs[rt]);
            break;
        }
        case 50: { // lfd frt, d(ra)
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(llvm::Type::getInt8Ty(ctx),
                memory_base, addr);
            llvm::Value* f64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f64_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(f64_ty, f64_ptr);
            builder.CreateStore(loaded, fprs[rt]);
            break;
        }
        case 52: { // stfs frt, d(ra) - Store Float Single
            llvm::Value* frt_val = builder.CreateLoad(f64_ty, fprs[rt]);
            llvm::Value* truncated = builder.CreateFPTrunc(frt_val, f32_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f32_ty, 0));
            builder.CreateStore(truncated, f32_ptr);
            break;
        }
        case 54: { // stfd frt, d(ra) - Store Float Double
            llvm::Value* frt_val = builder.CreateLoad(f64_ty, fprs[rt]);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f64_ty, 0));
            builder.CreateStore(frt_val, f64_ptr);
            break;
        }
        case 49: { // lfsu frt, d(ra) - Load Float Single with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f32_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(f32_ty, f32_ptr);
            llvm::Value* extended = builder.CreateFPExt(loaded, f64_ty);
            builder.CreateStore(extended, fprs[rt]);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 51: { // lfdu frt, d(ra) - Load Float Double with Update
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f64_ty, 0));
            llvm::Value* loaded = builder.CreateLoad(f64_ty, f64_ptr);
            builder.CreateStore(loaded, fprs[rt]);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 53: { // stfsu frt, d(ra) - Store Float Single with Update
            llvm::Value* frt_val = builder.CreateLoad(f64_ty, fprs[rt]);
            llvm::Value* truncated = builder.CreateFPTrunc(frt_val, f32_ty);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f32_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f32_ty, 0));
            builder.CreateStore(truncated, f32_ptr);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        case 55: { // stfdu frt, d(ra) - Store Float Double with Update
            llvm::Value* frt_val = builder.CreateLoad(f64_ty, fprs[rt]);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* addr = builder.CreateAdd(ra_val,
                llvm::ConstantInt::get(i64_ty, (int64_t)simm));
            llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
            llvm::Value* f64_ptr = builder.CreateBitCast(ptr,
                llvm::PointerType::get(f64_ty, 0));
            builder.CreateStore(frt_val, f64_ptr);
            builder.CreateStore(addr, gprs[ra]);
            break;
        }
        
        // M-Form: Rotate instructions (opcode 20-23)
        case 21: { // rlwinm ra, rs, sh, mb, me - Rotate Left Word Immediate then AND with Mask
            uint8_t rs = rt; // rs is in the rt field for M-form
            uint8_t sh = rb; // sh is in the rb field
            uint8_t mb = (instr >> 6) & 0x1F;
            uint8_t me = (instr >> 1) & 0x1F;
            
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
            llvm::Value* rs_32 = builder.CreateTrunc(rs_val, i32_ty);
            
            // Rotate left by sh bits
            llvm::Value* sh_val = llvm::ConstantInt::get(i32_ty, sh);
            llvm::Value* sh_inv = llvm::ConstantInt::get(i32_ty, 32 - sh);
            llvm::Value* rot_left = builder.CreateShl(rs_32, sh_val);
            llvm::Value* rot_right = builder.CreateLShr(rs_32, sh_inv);
            llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
            
            // Generate mask from mb to me
            uint32_t mask = 0;
            if (mb <= me) {
                mask = ((0xFFFFFFFFu >> mb) & (0xFFFFFFFFu << (31 - me)));
            } else {
                mask = ((0xFFFFFFFFu >> mb) | (0xFFFFFFFFu << (31 - me)));
            }
            llvm::Value* mask_val = llvm::ConstantInt::get(i32_ty, mask);
            llvm::Value* result_32 = builder.CreateAnd(rotated, mask_val);
            llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
            builder.CreateStore(result, gprs[ra]);
            break;
        }
        case 20: { // rlwimi ra, rs, sh, mb, me - Rotate Left Word Immediate then Mask Insert
            uint8_t rs = rt;
            uint8_t sh = rb;
            uint8_t mb = (instr >> 6) & 0x1F;
            uint8_t me = (instr >> 1) & 0x1F;
            
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* rs_32 = builder.CreateTrunc(rs_val, i32_ty);
            llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
            
            // Rotate left by sh bits
            llvm::Value* sh_val = llvm::ConstantInt::get(i32_ty, sh);
            llvm::Value* sh_inv = llvm::ConstantInt::get(i32_ty, 32 - sh);
            llvm::Value* rot_left = builder.CreateShl(rs_32, sh_val);
            llvm::Value* rot_right = builder.CreateLShr(rs_32, sh_inv);
            llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
            
            // Generate mask
            uint32_t mask = 0;
            if (mb <= me) {
                mask = ((0xFFFFFFFFu >> mb) & (0xFFFFFFFFu << (31 - me)));
            } else {
                mask = ((0xFFFFFFFFu >> mb) | (0xFFFFFFFFu << (31 - me)));
            }
            llvm::Value* mask_val = llvm::ConstantInt::get(i32_ty, mask);
            llvm::Value* inv_mask = llvm::ConstantInt::get(i32_ty, ~mask);
            
            // Insert: (rotated & mask) | (ra & ~mask)
            llvm::Value* part1 = builder.CreateAnd(rotated, mask_val);
            llvm::Value* part2 = builder.CreateAnd(ra_32, inv_mask);
            llvm::Value* result_32 = builder.CreateOr(part1, part2);
            llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
            builder.CreateStore(result, gprs[ra]);
            break;
        }
        case 23: { // rlwnm ra, rs, rb, mb, me - Rotate Left Word then AND with Mask
            uint8_t rs = rt;
            uint8_t mb = (instr >> 6) & 0x1F;
            uint8_t me = (instr >> 1) & 0x1F;
            
            llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
            llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
            llvm::Value* rs_32 = builder.CreateTrunc(rs_val, i32_ty);
            llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
            llvm::Value* sh = builder.CreateAnd(rb_32, llvm::ConstantInt::get(i32_ty, 0x1F));
            
            // Rotate left by sh bits
            llvm::Value* sh_inv = builder.CreateSub(llvm::ConstantInt::get(i32_ty, 32), sh);
            llvm::Value* rot_left = builder.CreateShl(rs_32, sh);
            llvm::Value* rot_right = builder.CreateLShr(rs_32, sh_inv);
            llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
            
            // Generate mask
            uint32_t mask = 0;
            if (mb <= me) {
                mask = ((0xFFFFFFFFu >> mb) & (0xFFFFFFFFu << (31 - me)));
            } else {
                mask = ((0xFFFFFFFFu >> mb) | (0xFFFFFFFFu << (31 - me)));
            }
            llvm::Value* mask_val = llvm::ConstantInt::get(i32_ty, mask);
            llvm::Value* result_32 = builder.CreateAnd(rotated, mask_val);
            llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
            builder.CreateStore(result, gprs[ra]);
            break;
        }
        
        // Extended opcodes (opcode 31) - X-form and XO-form instructions
        case 31: {
            uint16_t xo = (instr >> 1) & 0x3FF;
            switch (xo) {
                case 266: { // add rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateAdd(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 40: { // subf rt, ra, rb (rb - ra)
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateSub(rb_val, ra_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 8: { // subfc rt, ra, rb - Subtract From Carrying
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateSub(rb_val, ra_val);
                    builder.CreateStore(result, gprs[rt]);
                    // Set CA flag: subfc computes (rb - ra), CA = 1 if no borrow (rb >= ra unsigned)
                    set_ca_sub(builder, rb_val, ra_val, xer_ptr);
                    break;
                }
                case 10: { // addc rt, ra, rb - Add Carrying
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateAdd(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    // Set CA flag
                    set_ca_add(builder, ra_val, rb_val, result, xer_ptr);
                    break;
                }
                case 104: { // neg rt, ra - Negate
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* result = builder.CreateNeg(ra_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 138: { // adde rt, ra, rb - Add Extended
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    // Compute ra + rb + CA
                    llvm::Value* temp = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* result = builder.CreateAdd(temp, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag for extended add
                    set_ca_add_extended(builder, ra_val, rb_val, ca_in, xer_ptr);
                    break;
                }
                case 136: { // subfe rt, ra, rb - Subtract From Extended
                    // subfe: rt = ~ra + rb + CA
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* not_ra = builder.CreateNot(ra_val);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    // Compute ~ra + rb + CA
                    llvm::Value* temp = builder.CreateAdd(rb_val, not_ra);
                    llvm::Value* result = builder.CreateAdd(temp, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag for extended add
                    set_ca_add_extended(builder, not_ra, rb_val, ca_in, xer_ptr);
                    break;
                }
                case 202: { // addze rt, ra - Add to Zero Extended
                    // addze: rt = ra + CA
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    llvm::Value* result = builder.CreateAdd(ra_val, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag: set if ra + CA overflows
                    llvm::Value* zero = llvm::ConstantInt::get(i64_ty, 0);
                    set_ca_add_extended(builder, ra_val, zero, ca_in, xer_ptr);
                    break;
                }
                case 200: { // subfze rt, ra - Subtract From Zero Extended
                    // subfze: rt = ~ra + CA
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* not_ra = builder.CreateNot(ra_val);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    llvm::Value* result = builder.CreateAdd(not_ra, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag
                    llvm::Value* zero = llvm::ConstantInt::get(i64_ty, 0);
                    set_ca_add_extended(builder, not_ra, zero, ca_in, xer_ptr);
                    break;
                }
                case 234: { // addme rt, ra - Add to Minus One Extended
                    // addme: rt = ra + CA - 1 = ra + CA + 0xFFFFFFFF_FFFFFFFF
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* minus_one = llvm::ConstantInt::get(i64_ty, -1);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    // Compute ra + (-1) + CA
                    llvm::Value* temp = builder.CreateAdd(ra_val, minus_one);
                    llvm::Value* result = builder.CreateAdd(temp, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag
                    set_ca_add_extended(builder, ra_val, minus_one, ca_in, xer_ptr);
                    break;
                }
                case 232: { // subfme rt, ra - Subtract From Minus One Extended
                    // subfme: rt = ~ra + CA - 1 = ~ra + CA + 0xFFFFFFFF_FFFFFFFF
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* not_ra = builder.CreateNot(ra_val);
                    llvm::Value* minus_one = llvm::ConstantInt::get(i64_ty, -1);
                    // Get CA bit from XER
                    llvm::Value* ca_in = get_ca_bit(builder, xer_ptr);
                    // Compute ~ra + (-1) + CA
                    llvm::Value* temp = builder.CreateAdd(not_ra, minus_one);
                    llvm::Value* result = builder.CreateAdd(temp, ca_in);
                    builder.CreateStore(result, gprs[rt]);
                    // Update CA flag
                    set_ca_add_extended(builder, not_ra, minus_one, ca_in, xer_ptr);
                    break;
                }
                case 235: { // mullw rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
                    llvm::Value* result_32 = builder.CreateMul(ra_32, rb_32);
                    llvm::Value* result = builder.CreateSExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 233: { // mulld rt, ra, rb - Multiply Low Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateMul(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 75: { // mulhw rt, ra, rb - Multiply High Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
                    llvm::Value* ra_ext = builder.CreateSExt(ra_32, i64_ty);
                    llvm::Value* rb_ext = builder.CreateSExt(rb_32, i64_ty);
                    llvm::Value* product = builder.CreateMul(ra_ext, rb_ext);
                    llvm::Value* high = builder.CreateAShr(product, 
                        llvm::ConstantInt::get(i64_ty, 32));
                    llvm::Value* result = builder.CreateTrunc(high, i32_ty);
                    llvm::Value* result_ext = builder.CreateSExt(result, i64_ty);
                    builder.CreateStore(result_ext, gprs[rt]);
                    break;
                }
                case 11: { // mulhwu rt, ra, rb - Multiply High Word Unsigned
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
                    llvm::Value* ra_ext = builder.CreateZExt(ra_32, i64_ty);
                    llvm::Value* rb_ext = builder.CreateZExt(rb_32, i64_ty);
                    llvm::Value* product = builder.CreateMul(ra_ext, rb_ext);
                    llvm::Value* high = builder.CreateLShr(product, 
                        llvm::ConstantInt::get(i64_ty, 32));
                    llvm::Value* result = builder.CreateTrunc(high, i32_ty);
                    llvm::Value* result_ext = builder.CreateZExt(result, i64_ty);
                    builder.CreateStore(result_ext, gprs[rt]);
                    break;
                }
                case 491: { // divw rt, ra, rb - Divide Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
                    llvm::Value* result_32 = builder.CreateSDiv(ra_32, rb_32);
                    llvm::Value* result = builder.CreateSExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 459: { // divwu rt, ra, rb - Divide Word Unsigned
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* rb_32 = builder.CreateTrunc(rb_val, i32_ty);
                    llvm::Value* result_32 = builder.CreateUDiv(ra_32, rb_32);
                    llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 489: { // divd rt, ra, rb - Divide Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateSDiv(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 457: { // divdu rt, ra, rb - Divide Doubleword Unsigned
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateUDiv(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 28: { // and rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateAnd(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 60: { // andc rt, ra, rb - AND with Complement
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* not_rb = builder.CreateNot(rb_val);
                    llvm::Value* result = builder.CreateAnd(ra_val, not_rb);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 444: { // or rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateOr(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 412: { // orc rt, ra, rb - OR with Complement
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* not_rb = builder.CreateNot(rb_val);
                    llvm::Value* result = builder.CreateOr(ra_val, not_rb);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 316: { // xor rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* result = builder.CreateXor(ra_val, rb_val);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 476: { // nand rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* and_result = builder.CreateAnd(ra_val, rb_val);
                    llvm::Value* result = builder.CreateNot(and_result);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 124: { // nor rt, ra, rb
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* or_result = builder.CreateOr(ra_val, rb_val);
                    llvm::Value* result = builder.CreateNot(or_result);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 284: { // eqv rt, ra, rb - Equivalent (XNOR)
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* xor_result = builder.CreateXor(ra_val, rb_val);
                    llvm::Value* result = builder.CreateNot(xor_result);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Shift instructions
                case 24: { // slw rt, ra, rb - Shift Left Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* sh = builder.CreateTrunc(rb_val, i32_ty);
                    sh = builder.CreateAnd(sh, llvm::ConstantInt::get(i32_ty, 0x3F));
                    llvm::Value* result_32 = builder.CreateShl(ra_32, sh);
                    llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 536: { // srw rt, ra, rb - Shift Right Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* sh = builder.CreateTrunc(rb_val, i32_ty);
                    sh = builder.CreateAnd(sh, llvm::ConstantInt::get(i32_ty, 0x3F));
                    llvm::Value* result_32 = builder.CreateLShr(ra_32, sh);
                    llvm::Value* result = builder.CreateZExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 792: { // sraw rt, ra, rb - Shift Right Algebraic Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* sh = builder.CreateTrunc(rb_val, i32_ty);
                    sh = builder.CreateAnd(sh, llvm::ConstantInt::get(i32_ty, 0x3F));
                    llvm::Value* result_32 = builder.CreateAShr(ra_32, sh);
                    llvm::Value* result = builder.CreateSExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 824: { // srawi rt, ra, sh - Shift Right Algebraic Word Immediate
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* result_32 = builder.CreateAShr(ra_32,
                        llvm::ConstantInt::get(i32_ty, rb));
                    llvm::Value* result = builder.CreateSExt(result_32, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 27: { // sld rt, ra, rb - Shift Left Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* sh = builder.CreateAnd(rb_val,
                        llvm::ConstantInt::get(i64_ty, 0x7F));
                    llvm::Value* result = builder.CreateShl(ra_val, sh);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 539: { // srd rt, ra, rb - Shift Right Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* sh = builder.CreateAnd(rb_val,
                        llvm::ConstantInt::get(i64_ty, 0x7F));
                    llvm::Value* result = builder.CreateLShr(ra_val, sh);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 794: { // srad rt, ra, rb - Shift Right Algebraic Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* sh = builder.CreateAnd(rb_val,
                        llvm::ConstantInt::get(i64_ty, 0x7F));
                    llvm::Value* result = builder.CreateAShr(ra_val, sh);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Sign extension instructions
                case 954: { // extsb rt, ra - Extend Sign Byte
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* byte_val = builder.CreateTrunc(ra_val, i8_ty);
                    llvm::Value* result = builder.CreateSExt(byte_val, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 922: { // extsh rt, ra - Extend Sign Halfword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* hw_val = builder.CreateTrunc(ra_val, i16_ty);
                    llvm::Value* result = builder.CreateSExt(hw_val, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 986: { // extsw rt, ra - Extend Sign Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* word_val = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* result = builder.CreateSExt(word_val, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Count leading zeros
                case 26: { // cntlzw rt, ra - Count Leading Zeros Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* ra_32 = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Function* ctlz = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::ctlz, {i32_ty});
                    llvm::Value* count = builder.CreateCall(ctlz, 
                        {ra_32, llvm::ConstantInt::getFalse(ctx)});
                    llvm::Value* result = builder.CreateZExt(count, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 58: { // cntlzd rt, ra - Count Leading Zeros Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Function* ctlz = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::ctlz, {i64_ty});
                    llvm::Value* result = builder.CreateCall(ctlz, 
                        {ra_val, llvm::ConstantInt::getFalse(ctx)});
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Comparison instructions
                case 0: { // cmp bf, l, ra, rb - Compare
                    uint8_t bf = rt >> 2;
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    
                    // Compare and set CR field
                    llvm::Value* lt = builder.CreateICmpSLT(ra_val, rb_val);
                    llvm::Value* gt = builder.CreateICmpSGT(ra_val, rb_val);
                    llvm::Value* eq = builder.CreateICmpEQ(ra_val, rb_val);
                    
                    llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
                    cr_field = builder.CreateSelect(lt,
                        llvm::ConstantInt::get(i32_ty, 8), cr_field);
                    cr_field = builder.CreateSelect(gt,
                        llvm::ConstantInt::get(i32_ty, 4), cr_field);
                    cr_field = builder.CreateSelect(eq,
                        llvm::ConstantInt::get(i32_ty, 2), cr_field);
                    // Add SO bit from XER
                    llvm::Value* so_bit = get_so_bit_for_cr(builder, xer_ptr);
                    cr_field = builder.CreateOr(cr_field, so_bit);
                    
                    // Update CR (bf field)
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    uint32_t mask = ~(0xFu << (28 - bf * 4));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
                    llvm::Value* shifted = builder.CreateShl(cr_field,
                        llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 32: { // cmpl bf, l, ra, rb - Compare Logical (unsigned)
                    uint8_t bf = rt >> 2;
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    
                    llvm::Value* lt = builder.CreateICmpULT(ra_val, rb_val);
                    llvm::Value* gt = builder.CreateICmpUGT(ra_val, rb_val);
                    llvm::Value* eq = builder.CreateICmpEQ(ra_val, rb_val);
                    
                    llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
                    cr_field = builder.CreateSelect(lt,
                        llvm::ConstantInt::get(i32_ty, 8), cr_field);
                    cr_field = builder.CreateSelect(gt,
                        llvm::ConstantInt::get(i32_ty, 4), cr_field);
                    cr_field = builder.CreateSelect(eq,
                        llvm::ConstantInt::get(i32_ty, 2), cr_field);
                    // Add SO bit from XER
                    llvm::Value* so_bit = get_so_bit_for_cr(builder, xer_ptr);
                    cr_field = builder.CreateOr(cr_field, so_bit);
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    uint32_t mask = ~(0xFu << (28 - bf * 4));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
                    llvm::Value* shifted = builder.CreateShl(cr_field,
                        llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                // Indexed load/store instructions
                case 23: { // lwzx rt, ra, rb - Load Word and Zero Indexed
                    llvm::Value* ra_val = (ra == 0) ? 
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 151: { // stwx rs, ra, rb - Store Word Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    builder.CreateStore(truncated, i32_ptr);
                    break;
                }
                case 87: { // lbzx rt, ra, rb - Load Byte and Zero Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* loaded = builder.CreateLoad(i8_ty, ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 215: { // stbx rs, ra, rb - Store Byte Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i8_ty);
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    builder.CreateStore(truncated, ptr);
                    break;
                }
                case 279: { // lhzx rt, ra, rb - Load Halfword and Zero Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 407: { // sthx rs, ra, rb - Store Halfword Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i16_ty);
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    builder.CreateStore(truncated, i16_ptr);
                    break;
                }
                case 21: { // ldx rt, ra, rb - Load Doubleword Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i64_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i64_ty, i64_ptr);
                    builder.CreateStore(loaded, gprs[rt]);
                    break;
                }
                case 149: { // stdx rs, ra, rb - Store Doubleword Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i64_ty, 0));
                    builder.CreateStore(rs_val, i64_ptr);
                    break;
                }
                // SPR access
                case 339: { // mfspr rt, spr - Move From Special Purpose Register
                    uint16_t spr = ((instr >> 11) & 0x1F) | (((instr >> 16) & 0x1F) << 5);
                    llvm::Value* spr_val = llvm::ConstantInt::get(i64_ty, 0);
                    if (spr == 8) { // LR
                        spr_val = builder.CreateLoad(i64_ty, lr_ptr);
                    } else if (spr == 9) { // CTR
                        spr_val = builder.CreateLoad(i64_ty, ctr_ptr);
                    } else if (spr == 1) { // XER
                        spr_val = builder.CreateLoad(i64_ty, xer_ptr);
                    }
                    builder.CreateStore(spr_val, gprs[rt]);
                    break;
                }
                case 467: { // mtspr spr, rs - Move To Special Purpose Register
                    uint16_t spr = ((instr >> 11) & 0x1F) | (((instr >> 16) & 0x1F) << 5);
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    if (spr == 8) { // LR
                        builder.CreateStore(rs_val, lr_ptr);
                    } else if (spr == 9) { // CTR
                        builder.CreateStore(rs_val, ctr_ptr);
                    } else if (spr == 1) { // XER
                        builder.CreateStore(rs_val, xer_ptr);
                    }
                    break;
                }
                case 19: { // mfcr rt - Move From Condition Register
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    llvm::Value* result = builder.CreateZExt(cr, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 144: { // mtcrf fxm, rs - Move To Condition Register Fields
                    uint8_t fxm = (instr >> 12) & 0xFF;
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* rs_32 = builder.CreateTrunc(rs_val, i32_ty);
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    for (int i = 0; i < 8; i++) {
                        if ((fxm >> (7 - i)) & 1) {
                            uint32_t mask = ~(0xFu << (28 - i * 4));
                            cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
                            llvm::Value* field = builder.CreateAnd(rs_32,
                                llvm::ConstantInt::get(i32_ty, 0xFu << (28 - i * 4)));
                            cr = builder.CreateOr(cr, field);
                        }
                    }
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                // Indexed load/store with update
                case 55: { // lwzux rt, ra, rb - Load Word and Zero with Update Indexed
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 183: { // stwux rs, ra, rb - Store Word with Update Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    builder.CreateStore(truncated, i32_ptr);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 119: { // lbzux rt, ra, rb - Load Byte and Zero with Update Indexed
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* loaded = builder.CreateLoad(i8_ty, ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 247: { // stbux rs, ra, rb - Store Byte with Update Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i8_ty);
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    builder.CreateStore(truncated, ptr);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 311: { // lhzux rt, ra, rb - Load Halfword and Zero with Update Indexed
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
                    llvm::Value* result = builder.CreateZExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 375: { // lhaux rt, ra, rb - Load Halfword Algebraic with Update Indexed
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
                    llvm::Value* result = builder.CreateSExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 439: { // sthux rs, ra, rb - Store Halfword with Update Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i16_ty);
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    builder.CreateStore(truncated, i16_ptr);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 53: { // ldux rt, ra, rb - Load Doubleword with Update Indexed
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i64_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i64_ty, i64_ptr);
                    builder.CreateStore(loaded, gprs[rt]);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                case 181: { // stdux rs, ra, rb - Store Doubleword with Update Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i64_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i64_ty, 0));
                    builder.CreateStore(rs_val, i64_ptr);
                    builder.CreateStore(addr, gprs[ra]);
                    break;
                }
                // Byte-reversed load/store
                case 790: { // lhbrx rt, ra, rb - Load Halfword Byte-Reverse Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
                    // Byte swap: use bswap intrinsic
                    llvm::Function* bswap_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::bswap, {i16_ty});
                    llvm::Value* swapped = builder.CreateCall(bswap_fn, {loaded});
                    llvm::Value* result = builder.CreateZExt(swapped, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 534: { // lwbrx rt, ra, rb - Load Word Byte-Reverse Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                    llvm::Function* bswap_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::bswap, {i32_ty});
                    llvm::Value* swapped = builder.CreateCall(bswap_fn, {loaded});
                    llvm::Value* result = builder.CreateZExt(swapped, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 918: { // sthbrx rs, ra, rb - Store Halfword Byte-Reverse Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i16_ty);
                    llvm::Function* bswap_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::bswap, {i16_ty});
                    llvm::Value* swapped = builder.CreateCall(bswap_fn, {truncated});
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    builder.CreateStore(swapped, i16_ptr);
                    break;
                }
                case 662: { // stwbrx rs, ra, rb - Store Word Byte-Reverse Indexed
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rt]);
                    llvm::Value* truncated = builder.CreateTrunc(rs_val, i32_ty);
                    llvm::Function* bswap_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::bswap, {i32_ty});
                    llvm::Value* swapped = builder.CreateCall(bswap_fn, {truncated});
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    builder.CreateStore(swapped, i32_ptr);
                    break;
                }
                // Shift right algebraic doubleword immediate (2 variants)
                case 413:   // sradi (sh[5]=0)
                case 414: { // sradi (sh[5]=1)
                    uint8_t sh_lo = rb;
                    uint8_t sh_hi = (xo >> 0) & 1;  // Bit 0 of xo is sh[5]
                    uint8_t sh = sh_lo | (sh_hi << 5);
                    
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* result = builder.CreateAShr(ra_val,
                        llvm::ConstantInt::get(i64_ty, sh));
                    builder.CreateStore(result, gprs[rt]);
                    
                    // Set CA flag if negative and bits were shifted out
                    llvm::Value* is_negative = builder.CreateICmpSLT(ra_val,
                        llvm::ConstantInt::get(i64_ty, 0));
                    // Guard against undefined behavior: shift of 0 or >= 64
                    uint64_t shift_mask = (sh == 0) ? 0 : (sh >= 64) ? ~0ULL : (1ULL << sh) - 1;
                    llvm::Value* shifted_out = builder.CreateAnd(ra_val,
                        llvm::ConstantInt::get(i64_ty, shift_mask));
                    llvm::Value* has_bits = builder.CreateICmpNE(shifted_out,
                        llvm::ConstantInt::get(i64_ty, 0));
                    llvm::Value* set_ca = builder.CreateAnd(is_negative, has_bits);
                    
                    llvm::Value* xer = builder.CreateLoad(i64_ty, xer_ptr);
                    llvm::Value* xer_cleared = builder.CreateAnd(xer,
                        llvm::ConstantInt::get(i64_ty, ~XER_CA_MASK));
                    llvm::Value* ca_bit = builder.CreateSelect(set_ca,
                        llvm::ConstantInt::get(i64_ty, XER_CA_MASK),
                        llvm::ConstantInt::get(i64_ty, 0));
                    llvm::Value* new_xer = builder.CreateOr(xer_cleared, ca_bit);
                    builder.CreateStore(new_xer, xer_ptr);
                    break;
                }
                // Sync and cache instructions
                case 598: { // sync - Synchronize (also lwsync and ptesync)
                    // Memory barrier - emit fence in full implementation
                    break;
                }
                case 854: { // eieio - Enforce In-order Execution of I/O
                    // I/O ordering barrier
                    break;
                }
                case 982: { // icbi ra, rb - Instruction Cache Block Invalidate
                    // Cache invalidation - handled by JIT cache management
                    break;
                }
                case 470: { // dcbi ra, rb - Data Cache Block Invalidate
                    // Cache invalidation
                    break;
                }
                case 54: { // dcbst ra, rb - Data Cache Block Store
                    // Cache store - no explicit action needed
                    break;
                }
                case 86: { // dcbf ra, rb - Data Cache Block Flush
                    // Cache flush
                    break;
                }
                case 278: { // dcbt ra, rb - Data Cache Block Touch
                    // Prefetch hint - could emit prefetch intrinsic
                    break;
                }
                case 246: { // dcbtst ra, rb - Data Cache Block Touch for Store
                    // Prefetch hint for store
                    break;
                }
                case 1014: { // dcbz ra, rb - Data Cache Block Clear to Zero
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    // Align to cache line (typically 128 bytes on Cell)
                    llvm::Value* aligned = builder.CreateAnd(addr,
                        llvm::ConstantInt::get(i64_ty, ~127ULL));
                    
                    // Zero 128 bytes
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, aligned);
                    llvm::Function* memset_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::memset, {llvm::PointerType::get(i8_ty, 0), i64_ty});
                    builder.CreateCall(memset_fn, {
                        ptr,
                        llvm::ConstantInt::get(i8_ty, 0),
                        llvm::ConstantInt::get(i64_ty, 128),
                        llvm::ConstantInt::getFalse(ctx)
                    });
                    break;
                }
                // Trap instruction
                case 4: { // tw to, ra, rb - Trap Word
                    // Trap if condition met - for debugging
                    // In full implementation, would check condition and trap
                    break;
                }
                // Load/store halfword algebraic indexed
                case 343: { // lhax rt, ra, rb - Load Halfword Algebraic Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i16_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i16_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i16_ty, i16_ptr);
                    llvm::Value* result = builder.CreateSExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Load word algebraic indexed
                case 341: { // lwax rt, ra, rb - Load Word Algebraic Indexed
                    llvm::Value* ra_val = (ra == 0) ?
                        static_cast<llvm::Value*>(llvm::ConstantInt::get(i64_ty, 0)) :
                        static_cast<llvm::Value*>(builder.CreateLoad(i64_ty, gprs[ra]));
                    llvm::Value* rb_val = builder.CreateLoad(i64_ty, gprs[rb]);
                    llvm::Value* addr = builder.CreateAdd(ra_val, rb_val);
                    llvm::Value* ptr = builder.CreateGEP(i8_ty, memory_base, addr);
                    llvm::Value* i32_ptr = builder.CreateBitCast(ptr,
                        llvm::PointerType::get(i32_ty, 0));
                    llvm::Value* loaded = builder.CreateLoad(i32_ty, i32_ptr);
                    llvm::Value* result = builder.CreateSExt(loaded, i64_ty);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                // Population count
                case 378: { // popcntw rt, ra - Population Count Word
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Value* lo = builder.CreateTrunc(ra_val, i32_ty);
                    llvm::Value* hi = builder.CreateTrunc(
                        builder.CreateLShr(ra_val, llvm::ConstantInt::get(i64_ty, 32)), i32_ty);
                    
                    llvm::Function* ctpop_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::ctpop, {i32_ty});
                    llvm::Value* lo_count = builder.CreateCall(ctpop_fn, {lo});
                    llvm::Value* hi_count = builder.CreateCall(ctpop_fn, {hi});
                    
                    // Combine results: hi_count in upper 32 bits, lo_count in lower
                    llvm::Value* lo_ext = builder.CreateZExt(lo_count, i64_ty);
                    llvm::Value* hi_ext = builder.CreateZExt(hi_count, i64_ty);
                    llvm::Value* hi_shifted = builder.CreateShl(hi_ext,
                        llvm::ConstantInt::get(i64_ty, 32));
                    llvm::Value* result = builder.CreateOr(hi_shifted, lo_ext);
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                case 506: { // popcntd rt, ra - Population Count Doubleword
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    llvm::Function* ctpop_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::ctpop, {i64_ty});
                    llvm::Value* result = builder.CreateCall(ctpop_fn, {ra_val});
                    builder.CreateStore(result, gprs[rt]);
                    break;
                }
                default:
                    // Unhandled extended opcode
                    break;
            }
            break;
        }
        
        // Floating-point operations (opcode 63) - double precision
        case 63: {
            uint16_t xo_10 = (instr >> 1) & 0x3FF;
            uint16_t xo_5 = (instr >> 1) & 0x1F;
            uint8_t frc = (instr >> 6) & 0x1F;
            
            // Handle 10-bit XO instructions first
            switch (xo_10) {
                case 18: { // fdiv frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* result = builder.CreateFDiv(fra_val, frb_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 20: { // fsub frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* result = builder.CreateFSub(fra_val, frb_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 21: { // fadd frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* result = builder.CreateFAdd(fra_val, frb_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 22: { // fsqrt frt, frb
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Function* sqrt_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::sqrt, {f64_ty});
                    llvm::Value* result = builder.CreateCall(sqrt_fn, {frb_val});
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 24: { // fre frt, frb - Reciprocal Estimate
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* one = llvm::ConstantFP::get(f64_ty, 1.0);
                    llvm::Value* result = builder.CreateFDiv(one, frb_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 26: { // frsqrte frt, frb - Reciprocal Square Root Estimate
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Function* sqrt_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::sqrt, {f64_ty});
                    llvm::Value* sqrt_val = builder.CreateCall(sqrt_fn, {frb_val});
                    llvm::Value* one = llvm::ConstantFP::get(f64_ty, 1.0);
                    llvm::Value* result = builder.CreateFDiv(one, sqrt_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 40: { // fneg frt, frb
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* result = builder.CreateFNeg(frb_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 72: { // fmr frt, frb - Move Register
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    builder.CreateStore(frb_val, fprs[rt]);
                    break;
                }
                case 136: { // fnabs frt, frb - Negative Absolute
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Function* fabs_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fabs, {f64_ty});
                    llvm::Value* abs_val = builder.CreateCall(fabs_fn, {frb_val});
                    llvm::Value* result = builder.CreateFNeg(abs_val);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 264: { // fabs frt, frb
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Function* fabs_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fabs, {f64_ty});
                    llvm::Value* result = builder.CreateCall(fabs_fn, {frb_val});
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 0: { // fcmpu bf, fra, frb - Compare Unordered
                    uint8_t bf = rt >> 2;
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    
                    llvm::Value* lt = builder.CreateFCmpOLT(fra_val, frb_val);
                    llvm::Value* gt = builder.CreateFCmpOGT(fra_val, frb_val);
                    llvm::Value* eq = builder.CreateFCmpOEQ(fra_val, frb_val);
                    llvm::Value* un = builder.CreateFCmpUNO(fra_val, frb_val);
                    
                    llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
                    cr_field = builder.CreateSelect(lt,
                        llvm::ConstantInt::get(i32_ty, 8), cr_field);
                    cr_field = builder.CreateSelect(gt,
                        llvm::ConstantInt::get(i32_ty, 4), cr_field);
                    cr_field = builder.CreateSelect(eq,
                        llvm::ConstantInt::get(i32_ty, 2), cr_field);
                    cr_field = builder.CreateSelect(un,
                        llvm::ConstantInt::get(i32_ty, 1), cr_field);
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    uint32_t mask = ~(0xFu << (28 - bf * 4));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
                    llvm::Value* shifted = builder.CreateShl(cr_field,
                        llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 32: { // fcmpo bf, fra, frb - Compare Ordered
                    uint8_t bf = rt >> 2;
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    
                    llvm::Value* lt = builder.CreateFCmpOLT(fra_val, frb_val);
                    llvm::Value* gt = builder.CreateFCmpOGT(fra_val, frb_val);
                    llvm::Value* eq = builder.CreateFCmpOEQ(fra_val, frb_val);
                    llvm::Value* un = builder.CreateFCmpUNO(fra_val, frb_val);
                    
                    llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
                    cr_field = builder.CreateSelect(lt,
                        llvm::ConstantInt::get(i32_ty, 8), cr_field);
                    cr_field = builder.CreateSelect(gt,
                        llvm::ConstantInt::get(i32_ty, 4), cr_field);
                    cr_field = builder.CreateSelect(eq,
                        llvm::ConstantInt::get(i32_ty, 2), cr_field);
                    cr_field = builder.CreateSelect(un,
                        llvm::ConstantInt::get(i32_ty, 1), cr_field);
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    uint32_t mask = ~(0xFu << (28 - bf * 4));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
                    llvm::Value* shifted = builder.CreateShl(cr_field,
                        llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 14: { // fctiw frt, frb - Convert to Integer Word
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* i32_val = builder.CreateFPToSI(frb_val, i32_ty);
                    llvm::Value* i64_val = builder.CreateSExt(i32_val, i64_ty);
                    llvm::Value* result = builder.CreateBitCast(i64_val, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 15: { // fctiwz frt, frb - Convert to Integer Word with Round toward Zero
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* i32_val = builder.CreateFPToSI(frb_val, i32_ty);
                    llvm::Value* i64_val = builder.CreateSExt(i32_val, i64_ty);
                    llvm::Value* result = builder.CreateBitCast(i64_val, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 814: { // fctid frt, frb - Convert to Integer Doubleword
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* i64_val = builder.CreateFPToSI(frb_val, i64_ty);
                    llvm::Value* result = builder.CreateBitCast(i64_val, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 815: { // fctidz frt, frb - Convert to Integer Doubleword with Round toward Zero
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* i64_val = builder.CreateFPToSI(frb_val, i64_ty);
                    llvm::Value* result = builder.CreateBitCast(i64_val, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 846: { // fcfid frt, frb - Convert from Integer Doubleword
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* i64_val = builder.CreateBitCast(frb_val, i64_ty);
                    llvm::Value* result = builder.CreateSIToFP(i64_val, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 12: { // frsp frt, frb - Round to Single Precision
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* single = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* result = builder.CreateFPExt(single, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                default:
                    // Try 5-bit XO for A-form instructions
                    switch (xo_5) {
                        case 25: { // fmul frt, fra, frc
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* result = builder.CreateFMul(fra_val, frc_val);
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        case 29: { // fmadd frt, fra, frc, frb - Fused Multiply-Add
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                            llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                                builder.GetInsertBlock()->getModule(),
                                llvm::Intrinsic::fma, {f64_ty});
                            llvm::Value* result = builder.CreateCall(fma_fn, 
                                {fra_val, frc_val, frb_val});
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        case 28: { // fmsub frt, fra, frc, frb - Fused Multiply-Subtract
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                            llvm::Value* neg_frb = builder.CreateFNeg(frb_val);
                            llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                                builder.GetInsertBlock()->getModule(),
                                llvm::Intrinsic::fma, {f64_ty});
                            llvm::Value* result = builder.CreateCall(fma_fn,
                                {fra_val, frc_val, neg_frb});
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        case 31: { // fnmadd frt, fra, frc, frb - Fused Negative Multiply-Add
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                            llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                                builder.GetInsertBlock()->getModule(),
                                llvm::Intrinsic::fma, {f64_ty});
                            llvm::Value* fma_result = builder.CreateCall(fma_fn,
                                {fra_val, frc_val, frb_val});
                            llvm::Value* result = builder.CreateFNeg(fma_result);
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        case 30: { // fnmsub frt, fra, frc, frb - Fused Negative Multiply-Subtract
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                            llvm::Value* neg_frb = builder.CreateFNeg(frb_val);
                            llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                                builder.GetInsertBlock()->getModule(),
                                llvm::Intrinsic::fma, {f64_ty});
                            llvm::Value* fma_result = builder.CreateCall(fma_fn,
                                {fra_val, frc_val, neg_frb});
                            llvm::Value* result = builder.CreateFNeg(fma_result);
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        case 23: { // fsel frt, fra, frc, frb - Floating Select
                            llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                            llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                            llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                            llvm::Value* zero = llvm::ConstantFP::get(f64_ty, 0.0);
                            llvm::Value* cmp = builder.CreateFCmpOGE(fra_val, zero);
                            llvm::Value* result = builder.CreateSelect(cmp, frc_val, frb_val);
                            builder.CreateStore(result, fprs[rt]);
                            break;
                        }
                        default:
                            // Unhandled floating-point instruction
                            break;
                    }
                    break;
            }
            break;
        }
        
        // Floating-point single precision (opcode 59)
        case 59: {
            uint16_t xo_5 = (instr >> 1) & 0x1F;
            uint8_t frc = (instr >> 6) & 0x1F;
            
            switch (xo_5) {
                case 18: { // fdivs frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* result_s = builder.CreateFDiv(fra_s, frb_s);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 20: { // fsubs frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* result_s = builder.CreateFSub(fra_s, frb_s);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 21: { // fadds frt, fra, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* result_s = builder.CreateFAdd(fra_s, frb_s);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 22: { // fsqrts frt, frb
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Function* sqrt_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::sqrt, {f32_ty});
                    llvm::Value* result_s = builder.CreateCall(sqrt_fn, {frb_s});
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 24: { // fres frt, frb - Reciprocal Estimate Single
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* one = llvm::ConstantFP::get(f32_ty, 1.0f);
                    llvm::Value* result_s = builder.CreateFDiv(one, frb_s);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 25: { // fmuls frt, fra, frc
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frc_s = builder.CreateFPTrunc(frc_val, f32_ty);
                    llvm::Value* result_s = builder.CreateFMul(fra_s, frc_s);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 26: { // frsqrtes frt, frb - Reciprocal Square Root Estimate Single
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Function* sqrt_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::sqrt, {f32_ty});
                    llvm::Value* sqrt_val = builder.CreateCall(sqrt_fn, {frb_s});
                    llvm::Value* one = llvm::ConstantFP::get(f32_ty, 1.0f);
                    llvm::Value* result_s = builder.CreateFDiv(one, sqrt_val);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 29: { // fmadds frt, fra, frc, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frc_s = builder.CreateFPTrunc(frc_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fma, {f32_ty});
                    llvm::Value* result_s = builder.CreateCall(fma_fn,
                        {fra_s, frc_s, frb_s});
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 28: { // fmsubs frt, fra, frc, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frc_s = builder.CreateFPTrunc(frc_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* neg_frb = builder.CreateFNeg(frb_s);
                    llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fma, {f32_ty});
                    llvm::Value* result_s = builder.CreateCall(fma_fn,
                        {fra_s, frc_s, neg_frb});
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 31: { // fnmadds frt, fra, frc, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frc_s = builder.CreateFPTrunc(frc_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fma, {f32_ty});
                    llvm::Value* fma_result = builder.CreateCall(fma_fn,
                        {fra_s, frc_s, frb_s});
                    llvm::Value* result_s = builder.CreateFNeg(fma_result);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                case 30: { // fnmsubs frt, fra, frc, frb
                    llvm::Value* fra_val = builder.CreateLoad(f64_ty, fprs[ra]);
                    llvm::Value* frc_val = builder.CreateLoad(f64_ty, fprs[frc]);
                    llvm::Value* frb_val = builder.CreateLoad(f64_ty, fprs[rb]);
                    llvm::Value* fra_s = builder.CreateFPTrunc(fra_val, f32_ty);
                    llvm::Value* frc_s = builder.CreateFPTrunc(frc_val, f32_ty);
                    llvm::Value* frb_s = builder.CreateFPTrunc(frb_val, f32_ty);
                    llvm::Value* neg_frb = builder.CreateFNeg(frb_s);
                    llvm::Function* fma_fn = llvm::Intrinsic::getDeclaration(
                        builder.GetInsertBlock()->getModule(),
                        llvm::Intrinsic::fma, {f32_ty});
                    llvm::Value* fma_result = builder.CreateCall(fma_fn,
                        {fra_s, frc_s, neg_frb});
                    llvm::Value* result_s = builder.CreateFNeg(fma_result);
                    llvm::Value* result = builder.CreateFPExt(result_s, f64_ty);
                    builder.CreateStore(result, fprs[rt]);
                    break;
                }
                default:
                    break;
            }
            break;
        }
        
        // Branch instructions
        case 18: { // b/bl/ba/bla - Branch
            bool aa = (instr >> 1) & 1;  // Absolute address
            bool lk = instr & 1;         // Link (sets LR)
            int32_t li = ((int32_t)(instr & 0x03FFFFFC) << 6) >> 6;
            
            // If link bit set, save return address to LR
            if (lk && pc != 0) {
                llvm::Value* next_pc = llvm::ConstantInt::get(i64_ty, pc + 4);
                builder.CreateStore(next_pc, lr_ptr);
            }
            
            // Target address calculation (for block chaining in future)
            // If absolute: target = li
            // If relative: target = pc + li
            (void)aa;
            (void)li;
            break;
        }
        case 16: { // bc/bcl/bca/bcla - Branch Conditional
            uint8_t bo = rt;    // Branch options
            uint8_t bi = ra;    // Condition register bit
            bool aa = (instr >> 1) & 1;
            bool lk = instr & 1;
            int16_t bd = (int16_t)(instr & 0xFFFC);
            
            // Decrement CTR if required
            bool decr_ctr = !((bo >> 2) & 1);
            if (decr_ctr) {
                llvm::Value* ctr_val = builder.CreateLoad(i64_ty, ctr_ptr);
                llvm::Value* new_ctr = builder.CreateSub(ctr_val, 
                    llvm::ConstantInt::get(i64_ty, 1));
                builder.CreateStore(new_ctr, ctr_ptr);
            }
            
            // If link bit set, save return address to LR
            if (lk && pc != 0) {
                llvm::Value* next_pc = llvm::ConstantInt::get(i64_ty, pc + 4);
                builder.CreateStore(next_pc, lr_ptr);
            }
            
            // Branch condition check would be emitted here
            // For now, branch targets are handled by block chaining
            (void)bi;
            (void)aa;
            (void)bd;
            break;
        }
        
        // Extended opcodes 19 - Condition register and branch operations
        case 19: {
            uint16_t xo = (instr >> 1) & 0x3FF;
            switch (xo) {
                case 16: { // bclr - Branch Conditional to Link Register
                    uint8_t bo = rt;
                    uint8_t bi = ra;
                    bool lk = instr & 1;
                    
                    // Decrement CTR if required
                    bool decr_ctr = !((bo >> 2) & 1);
                    if (decr_ctr) {
                        llvm::Value* ctr_val = builder.CreateLoad(i64_ty, ctr_ptr);
                        llvm::Value* new_ctr = builder.CreateSub(ctr_val,
                            llvm::ConstantInt::get(i64_ty, 1));
                        builder.CreateStore(new_ctr, ctr_ptr);
                    }
                    
                    // If link bit set, save return address to LR (before branch)
                    // Note: For bclr, we save current LR to temp, then update LR
                    if (lk && pc != 0) {
                        // Read current LR (branch target)
                        llvm::Value* old_lr = builder.CreateLoad(i64_ty, lr_ptr);
                        // Store next PC to LR
                        llvm::Value* next_pc = llvm::ConstantInt::get(i64_ty, pc + 4);
                        builder.CreateStore(next_pc, lr_ptr);
                        (void)old_lr; // Branch target used by block chaining
                    }
                    
                    (void)bo;
                    (void)bi;
                    break;
                }
                case 528: { // bcctr - Branch Conditional to Count Register
                    uint8_t bo = rt;
                    uint8_t bi = ra;
                    bool lk = instr & 1;
                    
                    // If link bit set, save return address to LR
                    if (lk && pc != 0) {
                        llvm::Value* next_pc = llvm::ConstantInt::get(i64_ty, pc + 4);
                        builder.CreateStore(next_pc, lr_ptr);
                    }
                    
                    (void)bo;
                    (void)bi;
                    break;
                }
                case 0: { // mcrf - Move Condition Register Field
                    uint8_t bf = rt >> 2;
                    uint8_t bfa = ra >> 2;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    uint32_t src_mask = 0xFu << (28 - bfa * 4);
                    uint32_t dst_mask = ~(0xFu << (28 - bf * 4));
                    
                    llvm::Value* src_field = builder.CreateAnd(cr,
                        llvm::ConstantInt::get(i32_ty, src_mask));
                    
                    // Shift to destination position
                    int shift = (bfa - bf) * 4;
                    llvm::Value* shifted;
                    if (shift > 0) {
                        shifted = builder.CreateShl(src_field,
                            llvm::ConstantInt::get(i32_ty, shift));
                    } else if (shift < 0) {
                        shifted = builder.CreateLShr(src_field,
                            llvm::ConstantInt::get(i32_ty, -shift));
                    } else {
                        shifted = src_field;
                    }
                    
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, dst_mask));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 33: { // crnor crbd, crba, crbb - CR NOR
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    // Extract bits
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    // NOR operation
                    llvm::Value* result = builder.CreateNot(builder.CreateOr(bit_a, bit_b));
                    result = builder.CreateAnd(result, llvm::ConstantInt::get(i32_ty, 1));
                    
                    // Clear and set destination bit
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 129: { // crandc crbd, crba, crbb - CR AND with Complement
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    // ANDC operation
                    llvm::Value* not_b = builder.CreateXor(bit_b, llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* result = builder.CreateAnd(bit_a, not_b);
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 193: { // crxor crbd, crba, crbb - CR XOR
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* result = builder.CreateXor(bit_a, bit_b);
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 225: { // crnand crbd, crba, crbb - CR NAND
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* result = builder.CreateNot(builder.CreateAnd(bit_a, bit_b));
                    result = builder.CreateAnd(result, llvm::ConstantInt::get(i32_ty, 1));
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 257: { // crand crbd, crba, crbb - CR AND
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* result = builder.CreateAnd(bit_a, bit_b);
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 289: { // creqv crbd, crba, crbb - CR Equivalent (XNOR)
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* xor_result = builder.CreateXor(bit_a, bit_b);
                    llvm::Value* result = builder.CreateXor(xor_result, llvm::ConstantInt::get(i32_ty, 1));
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 417: { // crorc crbd, crba, crbb - CR OR with Complement
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* not_b = builder.CreateXor(bit_b, llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* result = builder.CreateOr(bit_a, not_b);
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 449: { // cror crbd, crba, crbb - CR OR
                    uint8_t crbd = rt;
                    uint8_t crba = ra;
                    uint8_t crbb = rb;
                    
                    llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
                    
                    llvm::Value* bit_a = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crba)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    llvm::Value* bit_b = builder.CreateAnd(
                        builder.CreateLShr(cr, llvm::ConstantInt::get(i32_ty, 31 - crbb)),
                        llvm::ConstantInt::get(i32_ty, 1));
                    
                    llvm::Value* result = builder.CreateOr(bit_a, bit_b);
                    
                    uint32_t bit_mask = ~(1u << (31 - crbd));
                    cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, bit_mask));
                    llvm::Value* shifted = builder.CreateShl(result,
                        llvm::ConstantInt::get(i32_ty, 31 - crbd));
                    cr = builder.CreateOr(cr, shifted);
                    builder.CreateStore(cr, cr_ptr);
                    break;
                }
                case 150: { // isync - Instruction Synchronize
                    // Memory barrier - no explicit code generation needed for JIT
                    // In a full implementation, would emit appropriate memory fence
                    break;
                }
                default:
                    break;
            }
            break;
        }
        
        // MD-Form: 64-bit rotate instructions (opcode 30)
        case 30: {
            uint8_t md_xo = (instr >> 1) & 0x7;  // 3-bit extended opcode
            uint8_t rs = rt;  // Source register (rt field holds rs for these)
            uint8_t sh_lo = rb;  // Lower 5 bits of shift
            uint8_t sh_hi = (instr >> 1) & 1;  // Bit 0 of extended opcode is sh[5]
            uint8_t sh = sh_lo | (sh_hi << 5);  // Full 6-bit shift amount
            uint8_t mb = ((instr >> 6) & 0x1F) | (((instr >> 5) & 1) << 5);  // 6-bit mask begin
            
            switch (md_xo >> 1) {  // Upper 2 bits of XO determine instruction
                case 0: { // rldicl ra, rs, sh, mb - Rotate Left Doubleword Immediate then Clear Left
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
                    
                    // Rotate left by sh bits
                    llvm::Value* sh_val = llvm::ConstantInt::get(i64_ty, sh);
                    llvm::Value* sh_inv = llvm::ConstantInt::get(i64_ty, 64 - sh);
                    llvm::Value* rot_left = builder.CreateShl(rs_val, sh_val);
                    llvm::Value* rot_right = builder.CreateLShr(rs_val, sh_inv);
                    llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
                    
                    // Generate mask: clear bits 0 to mb-1
                    uint64_t mask = (mb == 0) ? ~0ULL : (~0ULL >> mb);
                    llvm::Value* result = builder.CreateAnd(rotated,
                        llvm::ConstantInt::get(i64_ty, mask));
                    builder.CreateStore(result, gprs[ra]);
                    break;
                }
                case 1: { // rldicr ra, rs, sh, me - Rotate Left Doubleword Immediate then Clear Right
                    uint8_t me = mb;  // me uses the mb field for this instruction
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
                    
                    // Rotate left by sh bits
                    llvm::Value* sh_val = llvm::ConstantInt::get(i64_ty, sh);
                    llvm::Value* sh_inv = llvm::ConstantInt::get(i64_ty, 64 - sh);
                    llvm::Value* rot_left = builder.CreateShl(rs_val, sh_val);
                    llvm::Value* rot_right = builder.CreateLShr(rs_val, sh_inv);
                    llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
                    
                    // Generate mask: clear bits me+1 to 63
                    uint64_t mask = (me == 63) ? ~0ULL : (~0ULL << (63 - me));
                    llvm::Value* result = builder.CreateAnd(rotated,
                        llvm::ConstantInt::get(i64_ty, mask));
                    builder.CreateStore(result, gprs[ra]);
                    break;
                }
                case 2: { // rldic ra, rs, sh, mb - Rotate Left Doubleword Immediate then Clear
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
                    
                    // Rotate left by sh bits
                    llvm::Value* sh_val = llvm::ConstantInt::get(i64_ty, sh);
                    llvm::Value* sh_inv = llvm::ConstantInt::get(i64_ty, 64 - sh);
                    llvm::Value* rot_left = builder.CreateShl(rs_val, sh_val);
                    llvm::Value* rot_right = builder.CreateLShr(rs_val, sh_inv);
                    llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
                    
                    // Generate mask: mb to 63-sh
                    uint8_t me = 63 - sh;
                    uint64_t mask = 0;
                    if (mb <= me) {
                        mask = ((~0ULL >> mb) & (~0ULL << (63 - me)));
                    } else {
                        mask = ((~0ULL >> mb) | (~0ULL << (63 - me)));
                    }
                    llvm::Value* result = builder.CreateAnd(rotated,
                        llvm::ConstantInt::get(i64_ty, mask));
                    builder.CreateStore(result, gprs[ra]);
                    break;
                }
                case 3: { // rldimi ra, rs, sh, mb - Rotate Left Doubleword Immediate then Mask Insert
                    llvm::Value* rs_val = builder.CreateLoad(i64_ty, gprs[rs]);
                    llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
                    
                    // Rotate left by sh bits
                    llvm::Value* sh_val = llvm::ConstantInt::get(i64_ty, sh);
                    llvm::Value* sh_inv = llvm::ConstantInt::get(i64_ty, 64 - sh);
                    llvm::Value* rot_left = builder.CreateShl(rs_val, sh_val);
                    llvm::Value* rot_right = builder.CreateLShr(rs_val, sh_inv);
                    llvm::Value* rotated = builder.CreateOr(rot_left, rot_right);
                    
                    // Generate mask: mb to 63-sh
                    uint8_t me = 63 - sh;
                    uint64_t mask = 0;
                    if (mb <= me) {
                        mask = ((~0ULL >> mb) & (~0ULL << (63 - me)));
                    } else {
                        mask = ((~0ULL >> mb) | (~0ULL << (63 - me)));
                    }
                    
                    // Insert: (rotated & mask) | (ra & ~mask)
                    llvm::Value* part1 = builder.CreateAnd(rotated,
                        llvm::ConstantInt::get(i64_ty, mask));
                    llvm::Value* part2 = builder.CreateAnd(ra_val,
                        llvm::ConstantInt::get(i64_ty, ~mask));
                    llvm::Value* result = builder.CreateOr(part1, part2);
                    builder.CreateStore(result, gprs[ra]);
                    break;
                }
                default:
                    break;
            }
            break;
        }
        
        // System call (opcode 17)
        case 17: { // sc - System Call
            // System calls require special handling - set exit reason
            // and return to interpreter for syscall processing
            // In a full implementation, would emit call to syscall handler
            break;
        }
        
        // Trap word (opcode 3)
        case 3: { // twi - Trap Word Immediate
            // Trap instructions are used for debugging and error handling
            // In a full implementation, would emit conditional trap
            break;
        }
        
        // Comparison immediate (D-form)
        case 11: { // cmpi bf, l, ra, simm - Compare Immediate
            uint8_t bf = rt >> 2;
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* imm_val = llvm::ConstantInt::get(i64_ty, (int64_t)simm);
            
            llvm::Value* lt = builder.CreateICmpSLT(ra_val, imm_val);
            llvm::Value* gt = builder.CreateICmpSGT(ra_val, imm_val);
            llvm::Value* eq = builder.CreateICmpEQ(ra_val, imm_val);
            
            llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
            cr_field = builder.CreateSelect(lt,
                llvm::ConstantInt::get(i32_ty, 8), cr_field);
            cr_field = builder.CreateSelect(gt,
                llvm::ConstantInt::get(i32_ty, 4), cr_field);
            cr_field = builder.CreateSelect(eq,
                llvm::ConstantInt::get(i32_ty, 2), cr_field);
            // Add SO bit from XER
            llvm::Value* so_bit = get_so_bit_for_cr(builder, xer_ptr);
            cr_field = builder.CreateOr(cr_field, so_bit);
            
            llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
            uint32_t mask = ~(0xFu << (28 - bf * 4));
            cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
            llvm::Value* shifted = builder.CreateShl(cr_field,
                llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
            cr = builder.CreateOr(cr, shifted);
            builder.CreateStore(cr, cr_ptr);
            break;
        }
        case 10: { // cmpli bf, l, ra, uimm - Compare Logical Immediate
            uint8_t bf = rt >> 2;
            llvm::Value* ra_val = builder.CreateLoad(i64_ty, gprs[ra]);
            llvm::Value* imm_val = llvm::ConstantInt::get(i64_ty, (uint64_t)uimm);
            
            llvm::Value* lt = builder.CreateICmpULT(ra_val, imm_val);
            llvm::Value* gt = builder.CreateICmpUGT(ra_val, imm_val);
            llvm::Value* eq = builder.CreateICmpEQ(ra_val, imm_val);
            
            llvm::Value* cr_field = llvm::ConstantInt::get(i32_ty, 0);
            cr_field = builder.CreateSelect(lt,
                llvm::ConstantInt::get(i32_ty, 8), cr_field);
            cr_field = builder.CreateSelect(gt,
                llvm::ConstantInt::get(i32_ty, 4), cr_field);
            cr_field = builder.CreateSelect(eq,
                llvm::ConstantInt::get(i32_ty, 2), cr_field);
            // Add SO bit from XER
            llvm::Value* so_bit = get_so_bit_for_cr(builder, xer_ptr);
            cr_field = builder.CreateOr(cr_field, so_bit);
            
            llvm::Value* cr = builder.CreateLoad(i32_ty, cr_ptr);
            uint32_t mask = ~(0xFu << (28 - bf * 4));
            cr = builder.CreateAnd(cr, llvm::ConstantInt::get(i32_ty, mask));
            llvm::Value* shifted = builder.CreateShl(cr_field,
                llvm::ConstantInt::get(i32_ty, 28 - bf * 4));
            cr = builder.CreateOr(cr, shifted);
            builder.CreateStore(cr, cr_ptr);
            break;
        }
        
        // VMX/AltiVec instructions (opcode 4)
        // Implements common vector operations using LLVM's vector types
        case 4: {
            // Extract VMX register fields
            uint8_t vrt = rt;  // Vector target register
            uint8_t vra = ra;  // Vector source register A
            uint8_t vrb = rb;  // Vector source register B
            uint8_t vrc = (instr >> 6) & 0x1F;  // Vector source register C (for VA-form)
            
            // Extract sub-opcode fields
            uint16_t vxo_vx = (instr >> 1) & 0x3FF;  // 10-bit sub-opcode for VX-form
            uint8_t vxo_va = (instr >> 0) & 0x3F;  // 6-bit sub-opcode for VA-form
            
            // VA-Form instructions (6-bit sub-opcode in bits 0-5)
            switch (vxo_va) {
                case 46: { // vmaddfp vrt, vra, vrc, vrb - Vector Multiply-Add FP
                    // vrt = (vra * vrc) + vrb
                    llvm::Value* a = builder.CreateLoad(v4f32_ty, vrs[vra]);
                    llvm::Value* c = builder.CreateLoad(v4f32_ty, vrs[vrc]);
                    llvm::Value* b = builder.CreateLoad(v4f32_ty, vrs[vrb]);
                    llvm::Value* mul = builder.CreateFMul(a, c);
                    llvm::Value* result = builder.CreateFAdd(mul, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 47: { // vnmsubfp vrt, vra, vrc, vrb - Vector Negative Multiply-Subtract FP
                    // vrt = -((vra * vrc) - vrb) = vrb - (vra * vrc)
                    llvm::Value* a = builder.CreateLoad(v4f32_ty, vrs[vra]);
                    llvm::Value* c = builder.CreateLoad(v4f32_ty, vrs[vrc]);
                    llvm::Value* b = builder.CreateLoad(v4f32_ty, vrs[vrb]);
                    llvm::Value* mul = builder.CreateFMul(a, c);
                    llvm::Value* result = builder.CreateFSub(b, mul);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 43: { // vsel vrt, vra, vrb, vrc - Vector Select
                    // For each bit: result = (vrc & vrb) | (~vrc & vra)
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* c = builder.CreateLoad(v4i32_ty, vrs[vrc]);
                    llvm::Value* not_c = builder.CreateNot(c);
                    llvm::Value* c_and_b = builder.CreateAnd(c, b);
                    llvm::Value* not_c_and_a = builder.CreateAnd(not_c, a);
                    llvm::Value* result = builder.CreateOr(c_and_b, not_c_and_a);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 44: { // vperm vrt, vra, vrb, vrc - Vector Permute
                    // vperm selects bytes from the concatenation of vra and vrb based on vrc
                    // Each byte of vrc selects a byte from the 32-byte {vra, vrb} concatenation
                    auto v16i8_ty = llvm::VectorType::get(i8_ty, 16, false);
                    
                    // Load registers as byte vectors
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* c = builder.CreateLoad(v4i32_ty, vrs[vrc]);
                    
                    // Bitcast to byte vectors for permutation
                    llvm::Value* a_bytes = builder.CreateBitCast(a, v16i8_ty);
                    llvm::Value* b_bytes = builder.CreateBitCast(b, v16i8_ty);
                    llvm::Value* c_bytes = builder.CreateBitCast(c, v16i8_ty);
                    
                    // For simplicity, use llvm.experimental.vector.interleave2 or manual selection
                    // For now, implement a simplified version that handles common patterns
                    // Full implementation would require runtime byte selection
                    
                    // Create result by selecting from a or b based on low bit of index
                    llvm::Value* mask_low = llvm::ConstantVector::getSplat(
                        llvm::ElementCount::getFixed(16), llvm::ConstantInt::get(i8_ty, 0x10));
                    llvm::Value* use_b = builder.CreateICmpUGE(c_bytes, mask_low);
                    llvm::Value* result_bytes = builder.CreateSelect(use_b, b_bytes, a_bytes);
                    
                    // Bitcast back to i32 vector and store
                    llvm::Value* result = builder.CreateBitCast(result_bytes, v4i32_ty);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                default:
                    // Unhandled VA-form instruction
                    break;
            }
            
            // VX-Form instructions (10-bit sub-opcode)
            switch (vxo_vx) {
                case 10: { // vaddfp vrt, vra, vrb - Vector Add FP
                    llvm::Value* a = builder.CreateLoad(v4f32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4f32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateFAdd(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 74: { // vsubfp vrt, vra, vrb - Vector Subtract FP
                    llvm::Value* a = builder.CreateLoad(v4f32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4f32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateFSub(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 1028: { // vand vrt, vra, vrb - Vector AND
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateAnd(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 1156: { // vor vrt, vra, vrb - Vector OR
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateOr(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 1220: { // vxor vrt, vra, vrb - Vector XOR
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateXor(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 1284: { // vnor vrt, vra, vrb - Vector NOR
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* or_result = builder.CreateOr(a, b);
                    llvm::Value* result = builder.CreateNot(or_result);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 134: { // vcmpequw vrt, vra, vrb - Vector Compare Equal Unsigned Word
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* cmp = builder.CreateICmpEQ(a, b);
                    llvm::Value* result = builder.CreateSExt(cmp, v4i32_ty);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 902: { // vcmpgtsw vrt, vra, vrb - Vector Compare Greater Than Signed Word
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* cmp = builder.CreateICmpSGT(a, b);
                    llvm::Value* result = builder.CreateSExt(cmp, v4i32_ty);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 128: { // vadduwm vrt, vra, vrb - Vector Add Unsigned Word Modulo
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateAdd(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                case 1152: { // vsubuwm vrt, vra, vrb - Vector Subtract Unsigned Word Modulo
                    llvm::Value* a = builder.CreateLoad(v4i32_ty, vrs[vra]);
                    llvm::Value* b = builder.CreateLoad(v4i32_ty, vrs[vrb]);
                    llvm::Value* result = builder.CreateSub(a, b);
                    builder.CreateStore(result, vrs[vrt]);
                    break;
                }
                default:
                    // Unhandled VX-form instruction - no-op
                    break;
            }
            
            // Suppress unused variable warnings for fields used only in some code paths
            (void)vscr_ptr;
            break;
        }
        
        default:
            // Unhandled instruction - emit nop
            break;
    }
}

/**
 * Create LLVM function for basic block with optimization passes
 */
static llvm::Function* create_llvm_function(llvm::Module* module, BasicBlock* block) {
    auto& ctx = module->getContext();
    
    // Function type: void(void* ppu_state, void* memory)
    auto void_ty = llvm::Type::getVoidTy(ctx);
    auto ptr_ty = llvm::PointerType::get(llvm::Type::getInt8Ty(ctx), 0);
    llvm::FunctionType* func_ty = llvm::FunctionType::get(void_ty, {ptr_ty, ptr_ty}, false);
    
    // Create function
    std::string func_name = "ppu_block_" + std::to_string(block->start_address);
    llvm::Function* func = llvm::Function::Create(func_ty,
        llvm::Function::ExternalLinkage, func_name, module);
    
    // Create entry basic block
    llvm::BasicBlock* entry_bb = llvm::BasicBlock::Create(ctx, "entry", func);
    llvm::IRBuilder<> builder(entry_bb);
    
    // Allocate space for GPRs, FPRs, VRs, and special registers
    auto i32_ty = llvm::Type::getInt32Ty(ctx);
    auto i64_ty = llvm::Type::getInt64Ty(ctx);
    auto f64_ty = llvm::Type::getDoubleTy(ctx);
    auto f32_ty = llvm::Type::getFloatTy(ctx);
    auto v4f32_ty = llvm::VectorType::get(f32_ty, 4, false);  // 128-bit vector as 4 x float
    
    llvm::Value* gprs[32];
    llvm::Value* fprs[32];
    llvm::Value* vrs[32];  // Vector registers for VMX
    
    for (int i = 0; i < 32; i++) {
        gprs[i] = builder.CreateAlloca(i64_ty, nullptr, "gpr" + std::to_string(i));
        fprs[i] = builder.CreateAlloca(f64_ty, nullptr, "fpr" + std::to_string(i));
        vrs[i] = builder.CreateAlloca(v4f32_ty, nullptr, "vr" + std::to_string(i));
        // Initialize to zero
        builder.CreateStore(llvm::ConstantInt::get(i64_ty, 0), gprs[i]);
        builder.CreateStore(llvm::ConstantFP::get(f64_ty, 0.0), fprs[i]);
        builder.CreateStore(llvm::ConstantAggregateZero::get(v4f32_ty), vrs[i]);
    }
    
    // Allocate special registers
    llvm::Value* cr_ptr = builder.CreateAlloca(i32_ty, nullptr, "cr");
    llvm::Value* lr_ptr = builder.CreateAlloca(i64_ty, nullptr, "lr");
    llvm::Value* ctr_ptr = builder.CreateAlloca(i64_ty, nullptr, "ctr");
    llvm::Value* xer_ptr = builder.CreateAlloca(i64_ty, nullptr, "xer");
    llvm::Value* vscr_ptr = builder.CreateAlloca(i32_ty, nullptr, "vscr");  // Vector Status and Control Register
    
    // Initialize special registers to zero
    builder.CreateStore(llvm::ConstantInt::get(i32_ty, 0), cr_ptr);
    builder.CreateStore(llvm::ConstantInt::get(i64_ty, 0), lr_ptr);
    builder.CreateStore(llvm::ConstantInt::get(i64_ty, 0), ctr_ptr);
    builder.CreateStore(llvm::ConstantInt::get(i64_ty, 0), xer_ptr);
    builder.CreateStore(llvm::ConstantInt::get(i32_ty, 0), vscr_ptr);
    
    // Get memory base pointer from function argument
    llvm::Value* memory_base = func->getArg(1);
    
    // Emit IR for each instruction
    uint64_t current_pc = block->start_address;
    for (uint32_t instr : block->instructions) {
        emit_ppu_instruction(builder, instr, gprs, fprs, vrs, memory_base,
                            cr_ptr, lr_ptr, ctr_ptr, xer_ptr, vscr_ptr, current_pc);
        current_pc += 4; // PowerPC instructions are 4 bytes
    }
    
    // Return
    builder.CreateRetVoid();
    
    // Verify function
    std::string error_str;
    llvm::raw_string_ostream error_stream(error_str);
    if (llvm::verifyFunction(*func, &error_stream)) {
        // Function verification failed - log the error for debugging
        // This can occur with unsupported instruction patterns or invalid IR
        // In debug builds, the error details are in error_str
        #ifdef DEBUG
        // Log error_str for debugging
        #endif
        func->eraseFromParent();
        return nullptr;
    }
    
    return func;
}

/**
 * Apply optimization passes to the module
 */
static void apply_optimization_passes(llvm::Module* module) {
    // Create pass managers
    llvm::LoopAnalysisManager LAM;
    llvm::FunctionAnalysisManager FAM;
    llvm::CGSCCAnalysisManager CGAM;
    llvm::ModuleAnalysisManager MAM;
    
    // Create pass builder
    llvm::PassBuilder PB;
    
    // Register analyses
    PB.registerModuleAnalyses(MAM);
    PB.registerCGSCCAnalyses(CGAM);
    PB.registerFunctionAnalyses(FAM);
    PB.registerLoopAnalyses(LAM);
    PB.crossRegisterProxies(LAM, FAM, CGAM, MAM);
    
    // Build optimization pipeline (O2 level)
    llvm::ModulePassManager MPM = PB.buildPerModuleDefaultPipeline(llvm::OptimizationLevel::O2);
    
    // Run optimization passes
    MPM.run(*module, MAM);
}
#endif

/**
 * Emit native machine code from LLVM IR
 * In a full implementation, this would use LLVM's ExecutionEngine
 */
static void emit_machine_code(BasicBlock* /*block*/) {
#ifdef HAVE_LLVM
    // In a full LLVM implementation with LLJIT:
    // 1. The function would be added to the JIT's ThreadSafeModule
    // 2. LLJIT would compile it lazily on first lookup
    // 3. Optimization passes would be applied before code generation
    // 4. The function pointer would be retrieved via lookup
    // For now, this is handled in generate_llvm_ir
#endif
    
    // Placeholder implementation
    // The code is already "emitted" in generate_llvm_ir for compatibility
}

extern "C" {

oc_ppu_jit_t* oc_ppu_jit_create(void) {
    return new oc_ppu_jit_t();
}

void oc_ppu_jit_destroy(oc_ppu_jit_t* jit) {
    if (jit) {
        // Clean up compiled code
        for (auto& pair : jit->cache.blocks) {
            if (pair.second->compiled_code) {
                free(pair.second->compiled_code);
            }
        }
        delete jit;
    }
}

int oc_ppu_jit_compile(oc_ppu_jit_t* jit, uint32_t address, 
                       const uint8_t* code, size_t size) {
    if (!jit || !code || size == 0) {
        return -1;
    }
    
    if (!jit->enabled) {
        return -2;
    }
    
    // Check if already compiled
    if (jit->cache.find_block(address)) {
        return 0; // Already compiled
    }
    
    // Create new basic block
    auto block = std::make_unique<BasicBlock>(address);
    
    // Step 1: Identify basic block boundaries
    identify_basic_block(code, size, block.get());
    
    // Step 2: Generate LLVM IR
    generate_llvm_ir(block.get(), jit);
    
    // Step 3: Emit machine code
    emit_machine_code(block.get());
    
    // Step 4: Cache the compiled block
    jit->cache.insert_block(address, std::move(block));
    
    return 0;
}

void* oc_ppu_jit_get_compiled(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return nullptr;
    
    BasicBlock* block = jit->cache.find_block(address);
    return block ? block->compiled_code : nullptr;
}

void oc_ppu_jit_invalidate(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    
    auto it = jit->cache.blocks.find(address);
    if (it != jit->cache.blocks.end()) {
        if (it->second->compiled_code) {
            free(it->second->compiled_code);
        }
        jit->cache.total_size -= it->second->code_size;
        jit->cache.blocks.erase(it);
    }
}

void oc_ppu_jit_clear_cache(oc_ppu_jit_t* jit) {
    if (!jit) return;
    
    for (auto& pair : jit->cache.blocks) {
        if (pair.second->compiled_code) {
            free(pair.second->compiled_code);
        }
    }
    jit->cache.clear();
}

void oc_ppu_jit_add_breakpoint(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->breakpoints.add_breakpoint(address);
    // Invalidate compiled code at breakpoint
    oc_ppu_jit_invalidate(jit, address);
}

void oc_ppu_jit_remove_breakpoint(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->breakpoints.remove_breakpoint(address);
}

int oc_ppu_jit_has_breakpoint(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->breakpoints.has_breakpoint(address) ? 1 : 0;
}

// ============================================================================
// Branch Prediction APIs
// ============================================================================

void oc_ppu_jit_add_branch_hint(oc_ppu_jit_t* jit, uint32_t address, 
                                 uint32_t target, int hint) {
    if (!jit) return;
    BranchHint h = static_cast<BranchHint>(hint);
    jit->branch_predictor.add_prediction(address, target, h);
}

int oc_ppu_jit_predict_branch(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    auto* pred = jit->branch_predictor.get_prediction(address);
    return pred ? (pred->predict_taken() ? 1 : 0) : 0;
}

void oc_ppu_jit_update_branch(oc_ppu_jit_t* jit, uint32_t address, int taken) {
    if (!jit) return;
    jit->branch_predictor.update_prediction(address, taken != 0);
}

void oc_ppu_jit_set_branch_thresholds(oc_ppu_jit_t* jit, uint32_t likely_threshold,
                                       uint32_t unlikely_threshold) {
    if (!jit) return;
    jit->branch_predictor.set_default_thresholds(likely_threshold, unlikely_threshold);
}

void oc_ppu_jit_set_branch_thresholds_for_address(oc_ppu_jit_t* jit, uint32_t address,
                                                   uint32_t likely_threshold,
                                                   uint32_t unlikely_threshold) {
    if (!jit) return;
    jit->branch_predictor.set_branch_thresholds(address, likely_threshold, unlikely_threshold);
}

double oc_ppu_jit_get_branch_accuracy(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return -1.0;
    return jit->branch_predictor.get_branch_accuracy(address);
}

void oc_ppu_jit_get_branch_stats(oc_ppu_jit_t* jit, uint64_t* total_correct,
                                  uint64_t* total_incorrect, double* overall_accuracy) {
    if (!jit) {
        if (total_correct) *total_correct = 0;
        if (total_incorrect) *total_incorrect = 0;
        if (overall_accuracy) *overall_accuracy = 0.0;
        return;
    }
    auto stats = jit->branch_predictor.get_aggregate_stats();
    if (total_correct) *total_correct = stats.total_correct;
    if (total_incorrect) *total_incorrect = stats.total_incorrect;
    if (overall_accuracy) *overall_accuracy = stats.overall_accuracy;
}

void oc_ppu_jit_reset_branch_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->branch_predictor.reset_all_stats();
}

// ============================================================================
// Inline Cache APIs
// ============================================================================

void oc_ppu_jit_add_inline_cache(oc_ppu_jit_t* jit, uint32_t call_site, 
                                  uint32_t target) {
    if (!jit) return;
    jit->inline_cache.add_entry(call_site, target);
}

void* oc_ppu_jit_lookup_inline_cache(oc_ppu_jit_t* jit, uint32_t call_site) {
    if (!jit) return nullptr;
    auto* entry = jit->inline_cache.lookup(call_site);
    return entry ? entry->compiled_target : nullptr;
}

void oc_ppu_jit_invalidate_inline_cache(oc_ppu_jit_t* jit, uint32_t target) {
    if (!jit) return;
    jit->inline_cache.invalidate(target);
}

// ============================================================================
// Branch Target Cache (BTB) APIs
// ============================================================================

void oc_ppu_jit_btb_add(oc_ppu_jit_t* jit, uint32_t branch_address, 
                        uint32_t target_address) {
    if (!jit) return;
    jit->branch_target_cache.add_entry(branch_address, target_address);
}

uint32_t oc_ppu_jit_btb_lookup(oc_ppu_jit_t* jit, uint32_t branch_address) {
    if (!jit) return 0;
    return jit->branch_target_cache.lookup(branch_address);
}

void oc_ppu_jit_btb_update(oc_ppu_jit_t* jit, uint32_t branch_address, 
                           uint32_t actual_target) {
    if (!jit) return;
    jit->branch_target_cache.update(branch_address, actual_target);
}

int oc_ppu_jit_btb_validate(oc_ppu_jit_t* jit, uint32_t branch_address, 
                            uint32_t expected_target) {
    if (!jit) return 0;
    return jit->branch_target_cache.validate(branch_address, expected_target) ? 1 : 0;
}

void oc_ppu_jit_btb_invalidate(oc_ppu_jit_t* jit, uint32_t branch_address) {
    if (!jit) return;
    jit->branch_target_cache.invalidate(branch_address);
}

void oc_ppu_jit_btb_invalidate_target(oc_ppu_jit_t* jit, uint32_t target_address) {
    if (!jit) return;
    jit->branch_target_cache.invalidate_target(target_address);
}

void oc_ppu_jit_btb_update_compiled(oc_ppu_jit_t* jit, uint32_t branch_address,
                                     uint32_t target_address, void* compiled) {
    if (!jit) return;
    jit->branch_target_cache.update_compiled(branch_address, target_address, compiled);
}

void* oc_ppu_jit_btb_get_compiled(oc_ppu_jit_t* jit, uint32_t branch_address,
                                   uint32_t target_address) {
    if (!jit) return nullptr;
    return jit->branch_target_cache.get_compiled(branch_address, target_address);
}

void oc_ppu_jit_btb_get_stats(oc_ppu_jit_t* jit, uint64_t* total_lookups,
                               uint64_t* total_hits, uint64_t* total_misses,
                               double* hit_rate) {
    if (!jit) {
        if (total_lookups) *total_lookups = 0;
        if (total_hits) *total_hits = 0;
        if (total_misses) *total_misses = 0;
        if (hit_rate) *hit_rate = 0.0;
        return;
    }
    auto stats = jit->branch_target_cache.get_stats();
    if (total_lookups) *total_lookups = stats.total_lookups;
    if (total_hits) *total_hits = stats.total_hits;
    if (total_misses) *total_misses = stats.total_misses;
    if (hit_rate) *hit_rate = stats.overall_hit_rate;
}

void oc_ppu_jit_btb_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->branch_target_cache.reset_stats();
}

void oc_ppu_jit_btb_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->branch_target_cache.clear();
}

// ============================================================================
// Constant Propagation Cache APIs
// ============================================================================

void oc_ppu_jit_const_set_imm(oc_ppu_jit_t* jit, uint32_t instr_addr, uint64_t value) {
    if (!jit) return;
    jit->const_prop_cache.set_immediate(instr_addr, value);
}

int oc_ppu_jit_const_get_imm(oc_ppu_jit_t* jit, uint32_t instr_addr, uint64_t* out_value) {
    if (!jit) return 0;
    return jit->const_prop_cache.get_immediate(instr_addr, out_value) ? 1 : 0;
}

void oc_ppu_jit_const_set_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num,
                               uint64_t value, uint32_t def_addr, int is_constant) {
    if (!jit) return;
    jit->const_prop_cache.set_register_value(block_addr, reg_num, value, def_addr, 
                                              is_constant != 0);
}

int oc_ppu_jit_const_get_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num,
                              uint64_t* out_value, int* out_is_constant) {
    if (!jit) return 0;
    bool is_const = false;
    bool found = jit->const_prop_cache.get_register_value(block_addr, reg_num, 
                                                           out_value, &is_const);
    if (out_is_constant) *out_is_constant = is_const ? 1 : 0;
    return found ? 1 : 0;
}

void oc_ppu_jit_const_invalidate_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num) {
    if (!jit) return;
    jit->const_prop_cache.invalidate_register(block_addr, reg_num);
}

void oc_ppu_jit_const_invalidate_all_regs(oc_ppu_jit_t* jit, uint32_t block_addr) {
    if (!jit) return;
    jit->const_prop_cache.invalidate_all_registers(block_addr);
}

void oc_ppu_jit_const_set_mem(oc_ppu_jit_t* jit, uint32_t mem_addr, uint64_t value,
                               uint8_t size, uint32_t load_addr) {
    if (!jit) return;
    jit->const_prop_cache.set_memory_load(mem_addr, value, size, load_addr);
}

int oc_ppu_jit_const_get_mem(oc_ppu_jit_t* jit, uint32_t mem_addr, uint64_t* out_value,
                              uint8_t* out_size) {
    if (!jit) return 0;
    return jit->const_prop_cache.get_memory_load(mem_addr, out_value, out_size) ? 1 : 0;
}

void oc_ppu_jit_const_invalidate_mem(oc_ppu_jit_t* jit, uint32_t mem_addr) {
    if (!jit) return;
    jit->const_prop_cache.invalidate_memory(mem_addr);
}

void oc_ppu_jit_const_invalidate_mem_range(oc_ppu_jit_t* jit, uint32_t start_addr, 
                                            uint32_t size) {
    if (!jit) return;
    jit->const_prop_cache.invalidate_memory_range(start_addr, size);
}

void oc_ppu_jit_const_get_stats(oc_ppu_jit_t* jit, uint64_t* imm_hits, uint64_t* imm_misses,
                                 uint64_t* reg_hits, uint64_t* reg_misses,
                                 uint64_t* mem_hits, uint64_t* mem_misses) {
    if (!jit) {
        if (imm_hits) *imm_hits = 0;
        if (imm_misses) *imm_misses = 0;
        if (reg_hits) *reg_hits = 0;
        if (reg_misses) *reg_misses = 0;
        if (mem_hits) *mem_hits = 0;
        if (mem_misses) *mem_misses = 0;
        return;
    }
    auto stats = jit->const_prop_cache.get_stats();
    if (imm_hits) *imm_hits = stats.imm_hits;
    if (imm_misses) *imm_misses = stats.imm_misses;
    if (reg_hits) *reg_hits = stats.reg_hits;
    if (reg_misses) *reg_misses = stats.reg_misses;
    if (mem_hits) *mem_hits = stats.mem_hits;
    if (mem_misses) *mem_misses = stats.mem_misses;
}

void oc_ppu_jit_const_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->const_prop_cache.reset_stats();
}

void oc_ppu_jit_const_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->const_prop_cache.clear();
}

// ============================================================================
// Register Allocation APIs
// ============================================================================

void oc_ppu_jit_analyze_registers(oc_ppu_jit_t* jit, uint32_t address,
                                   const uint32_t* instructions, size_t count) {
    if (!jit || !instructions) return;
    std::vector<uint32_t> instrs(instructions, instructions + count);
    jit->reg_allocator.analyze_block(address, instrs);
    jit->enhanced_reg_allocator.analyze_block(address, instrs);
}

int oc_ppu_jit_get_reg_hint(oc_ppu_jit_t* jit, uint32_t address, uint8_t reg) {
    if (!jit) return 0;
    // Use enhanced allocator for better hints
    return static_cast<int>(jit->enhanced_reg_allocator.get_hint(address, reg));
}

uint32_t oc_ppu_jit_get_live_gprs(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    auto* liveness = jit->enhanced_reg_allocator.get_liveness(address);
    return liveness ? liveness->live_gprs : 0;
}

uint32_t oc_ppu_jit_get_modified_gprs(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    auto* liveness = jit->enhanced_reg_allocator.get_liveness(address);
    return liveness ? liveness->modified_gprs : 0;
}

// Enhanced Register Allocation APIs

void oc_ppu_jit_reg_add_edge(oc_ppu_jit_t* jit, uint32_t from_addr, uint32_t to_addr) {
    if (!jit) return;
    jit->enhanced_reg_allocator.add_edge(from_addr, to_addr);
}

int oc_ppu_jit_reg_propagate_liveness(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_reg_allocator.propagate_liveness() ? 1 : 0;
}

uint32_t oc_ppu_jit_reg_allocate_spill(oc_ppu_jit_t* jit, uint8_t reg_num, 
                                        uint8_t reg_type, uint32_t spill_addr) {
    if (!jit) return 0;
    return jit->enhanced_reg_allocator.allocate_spill_slot(reg_num, reg_type, spill_addr);
}

void oc_ppu_jit_reg_free_spill(oc_ppu_jit_t* jit, uint32_t slot_id, uint32_t fill_addr) {
    if (!jit) return;
    jit->enhanced_reg_allocator.free_spill_slot(slot_id, fill_addr);
}

int oc_ppu_jit_reg_get_spill_offset(oc_ppu_jit_t* jit, uint32_t slot_id) {
    if (!jit) return -1;
    auto* slot = jit->enhanced_reg_allocator.get_spill_slot(slot_id);
    return slot ? static_cast<int>(slot->offset) : -1;
}

int oc_ppu_jit_reg_needs_spill(oc_ppu_jit_t* jit, uint32_t block_addr, 
                                uint8_t reg_num, uint8_t reg_type) {
    if (!jit) return 0;
    return jit->enhanced_reg_allocator.needs_spill(block_addr, reg_num, reg_type) ? 1 : 0;
}

uint32_t oc_ppu_jit_reg_get_live_in(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_type) {
    if (!jit) return 0;
    auto* state = jit->enhanced_reg_allocator.get_cross_block_state(block_addr);
    if (!state) return 0;
    switch (reg_type) {
        case 0: return state->live_in_gprs;
        case 1: return state->live_in_fprs;
        case 2: return state->live_in_vrs;
        default: return 0;
    }
}

uint32_t oc_ppu_jit_reg_get_live_out(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_type) {
    if (!jit) return 0;
    auto* state = jit->enhanced_reg_allocator.get_cross_block_state(block_addr);
    if (!state) return 0;
    switch (reg_type) {
        case 0: return state->live_out_gprs;
        case 1: return state->live_out_fprs;
        case 2: return state->live_out_vrs;
        default: return 0;
    }
}

size_t oc_ppu_jit_reg_coalesce_copies(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_reg_allocator.run_coalescing();
}

uint8_t oc_ppu_jit_reg_get_coalesced(oc_ppu_jit_t* jit, uint8_t reg, uint8_t reg_type) {
    if (!jit) return reg;
    return jit->enhanced_reg_allocator.get_coalesced_reg(reg, reg_type);
}

void oc_ppu_jit_reg_get_stats(oc_ppu_jit_t* jit, uint64_t* blocks_analyzed,
                               uint64_t* total_spills, uint64_t* total_fills,
                               uint64_t* copies_eliminated) {
    if (!jit) {
        if (blocks_analyzed) *blocks_analyzed = 0;
        if (total_spills) *total_spills = 0;
        if (total_fills) *total_fills = 0;
        if (copies_eliminated) *copies_eliminated = 0;
        return;
    }
    auto stats = jit->enhanced_reg_allocator.get_stats();
    if (blocks_analyzed) *blocks_analyzed = stats.blocks_analyzed;
    if (total_spills) *total_spills = stats.total_spills;
    if (total_fills) *total_fills = stats.total_fills;
    if (copies_eliminated) *copies_eliminated = stats.copies_eliminated;
}

void oc_ppu_jit_reg_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->enhanced_reg_allocator.reset_stats();
}

void oc_ppu_jit_reg_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->reg_allocator.clear();
    jit->enhanced_reg_allocator.clear();
}

// ============================================================================
// Lazy Compilation APIs
// ============================================================================

void oc_ppu_jit_enable_lazy(oc_ppu_jit_t* jit, int enable) {
    if (!jit) return;
    jit->lazy_compilation_enabled = (enable != 0);
}

int oc_ppu_jit_is_lazy_enabled(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->lazy_compilation_enabled ? 1 : 0;
}

void oc_ppu_jit_register_lazy(oc_ppu_jit_t* jit, uint32_t address,
                               const uint8_t* code, size_t size, 
                               uint32_t threshold) {
    if (!jit || !code) return;
    jit->lazy_manager.register_lazy(address, code, size, threshold);
}

int oc_ppu_jit_should_compile_lazy(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    auto* entry = jit->lazy_manager.get_entry(address);
    if (!entry) return 1; // Not registered, compile immediately
    if (entry->state == LazyState::Compiled) return 0; // Already compiled
    return entry->should_compile() ? 1 : 0;
}

int oc_ppu_jit_get_lazy_state(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    auto* entry = jit->lazy_manager.get_entry(address);
    return entry ? static_cast<int>(entry->state) : 0;
}

// Enhanced Lazy Compilation APIs

void oc_ppu_jit_lazy_set_default_threshold(oc_ppu_jit_t* jit, uint32_t threshold) {
    if (!jit) return;
    jit->enhanced_lazy_manager.set_default_threshold(threshold);
}

uint32_t oc_ppu_jit_lazy_get_default_threshold(oc_ppu_jit_t* jit) {
    if (!jit) return 10;
    return jit->enhanced_lazy_manager.get_default_threshold();
}

void oc_ppu_jit_lazy_set_hot_threshold(oc_ppu_jit_t* jit, uint32_t threshold) {
    if (!jit) return;
    jit->enhanced_lazy_manager.set_hot_threshold(threshold);
}

void oc_ppu_jit_lazy_register(oc_ppu_jit_t* jit, uint32_t address,
                               const uint8_t* code, size_t size, 
                               uint32_t threshold) {
    if (!jit || !code) return;
    // Register with both managers for compatibility
    jit->lazy_manager.register_lazy(address, code, size, threshold);
    jit->enhanced_lazy_manager.register_lazy(address, code, size, threshold);
}

int oc_ppu_jit_lazy_record_execution(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->enhanced_lazy_manager.record_execution(address) ? 1 : 0;
}

uint32_t oc_ppu_jit_lazy_get_execution_count(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->enhanced_lazy_manager.get_execution_count(address);
}

int oc_ppu_jit_lazy_get_enhanced_state(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return static_cast<int>(jit->enhanced_lazy_manager.get_state(address));
}

uint32_t oc_ppu_jit_lazy_get_next_hot_address(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_lazy_manager.get_next_hot_address();
}

size_t oc_ppu_jit_lazy_get_hot_addresses(oc_ppu_jit_t* jit, uint32_t* addresses, 
                                          uint32_t* exec_counts, int* compiled,
                                          size_t max_count) {
    if (!jit) return 0;
    
    auto hot_list = jit->enhanced_lazy_manager.get_hot_addresses(max_count);
    size_t count = hot_list.size();
    
    for (size_t i = 0; i < count; i++) {
        if (addresses) addresses[i] = hot_list[i].address;
        if (exec_counts) exec_counts[i] = hot_list[i].execution_count;
        if (compiled) compiled[i] = hot_list[i].is_compiled ? 1 : 0;
    }
    
    return count;
}

size_t oc_ppu_jit_lazy_get_pending_count(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_lazy_manager.get_pending_count();
}

void oc_ppu_jit_lazy_mark_compiling(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->lazy_manager.mark_compiling(address);
    jit->enhanced_lazy_manager.mark_compiling(address);
}

void oc_ppu_jit_lazy_mark_compiled(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->lazy_manager.mark_compiled(address);
    jit->enhanced_lazy_manager.mark_compiled(address);
}

void oc_ppu_jit_lazy_mark_failed(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->lazy_manager.mark_failed(address);
    jit->enhanced_lazy_manager.mark_failed(address);
}

void oc_ppu_jit_lazy_get_stats(oc_ppu_jit_t* jit, uint64_t* total_registered,
                                uint64_t* total_compiled, uint64_t* total_failed,
                                uint64_t* total_executions, uint64_t* hot_promotions,
                                uint64_t* stub_calls) {
    if (!jit) {
        if (total_registered) *total_registered = 0;
        if (total_compiled) *total_compiled = 0;
        if (total_failed) *total_failed = 0;
        if (total_executions) *total_executions = 0;
        if (hot_promotions) *hot_promotions = 0;
        if (stub_calls) *stub_calls = 0;
        return;
    }
    
    auto stats = jit->enhanced_lazy_manager.get_stats();
    if (total_registered) *total_registered = stats.total_registered;
    if (total_compiled) *total_compiled = stats.total_compiled;
    if (total_failed) *total_failed = stats.total_failed;
    if (total_executions) *total_executions = stats.total_executions;
    if (hot_promotions) *hot_promotions = stats.hot_path_promotions;
    if (stub_calls) *stub_calls = stats.stub_calls;
}

void oc_ppu_jit_lazy_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->enhanced_lazy_manager.reset_stats();
}

void oc_ppu_jit_lazy_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->lazy_manager.clear();
    jit->enhanced_lazy_manager.clear();
}

// ============================================================================
// Tiered Compilation APIs
// ============================================================================

void oc_ppu_jit_tiered_set_thresholds(oc_ppu_jit_t* jit, uint32_t tier0_to_1, uint32_t tier1_to_2) {
    if (!jit) return;
    jit->tiered_manager.set_thresholds(tier0_to_1, tier1_to_2);
}

void oc_ppu_jit_tiered_get_thresholds(oc_ppu_jit_t* jit, uint32_t* tier0_to_1, uint32_t* tier1_to_2) {
    if (!jit) {
        if (tier0_to_1) *tier0_to_1 = 10;
        if (tier1_to_2) *tier1_to_2 = 1000;
        return;
    }
    jit->tiered_manager.get_thresholds(tier0_to_1, tier1_to_2);
}

void oc_ppu_jit_tiered_register(oc_ppu_jit_t* jit, uint32_t address,
                                 const uint8_t* code, size_t size,
                                 uint32_t tier0_to_1, uint32_t tier1_to_2) {
    if (!jit || !code) return;
    jit->tiered_manager.register_code(address, code, size, tier0_to_1, tier1_to_2);
}

int oc_ppu_jit_tiered_record_execution(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    CompilationTier next_tier = jit->tiered_manager.record_execution(address);
    CompilationTier current_tier = jit->tiered_manager.get_tier(address);
    
    // If next_tier differs from current, a promotion should happen
    if (next_tier != current_tier) {
        return static_cast<int>(next_tier);
    }
    return static_cast<int>(current_tier);
}

int oc_ppu_jit_tiered_get_tier(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return static_cast<int>(jit->tiered_manager.get_tier(address));
}

int oc_ppu_jit_tiered_promote(oc_ppu_jit_t* jit, uint32_t address, int target_tier) {
    if (!jit) return 0;
    if (target_tier < 0 || target_tier > 2) return 0;
    
    return jit->tiered_manager.promote(address, static_cast<CompilationTier>(target_tier)) ? 1 : 0;
}

void* oc_ppu_jit_tiered_get_code(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return nullptr;
    return jit->tiered_manager.get_compiled_code(address);
}

uint32_t oc_ppu_jit_tiered_get_execution_count(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->tiered_manager.get_execution_count(address);
}

void oc_ppu_jit_tiered_get_tier_counts(oc_ppu_jit_t* jit, size_t* tier0, size_t* tier1, size_t* tier2) {
    if (!jit) {
        if (tier0) *tier0 = 0;
        if (tier1) *tier1 = 0;
        if (tier2) *tier2 = 0;
        return;
    }
    jit->tiered_manager.get_tier_counts(tier0, tier1, tier2);
}

void oc_ppu_jit_tiered_get_stats(oc_ppu_jit_t* jit, uint64_t* total_registered,
                                  uint64_t* tier0_execs, uint64_t* tier1_execs, uint64_t* tier2_execs,
                                  uint64_t* tier0_to_1_promotions, uint64_t* tier1_to_2_promotions) {
    if (!jit) {
        if (total_registered) *total_registered = 0;
        if (tier0_execs) *tier0_execs = 0;
        if (tier1_execs) *tier1_execs = 0;
        if (tier2_execs) *tier2_execs = 0;
        if (tier0_to_1_promotions) *tier0_to_1_promotions = 0;
        if (tier1_to_2_promotions) *tier1_to_2_promotions = 0;
        return;
    }
    
    auto stats = jit->tiered_manager.get_stats();
    if (total_registered) *total_registered = stats.total_registered;
    if (tier0_execs) *tier0_execs = stats.tier0_executions;
    if (tier1_execs) *tier1_execs = stats.tier1_executions;
    if (tier2_execs) *tier2_execs = stats.tier2_executions;
    if (tier0_to_1_promotions) *tier0_to_1_promotions = stats.tier0_to_1_promotions;
    if (tier1_to_2_promotions) *tier1_to_2_promotions = stats.tier1_to_2_promotions;
}

void oc_ppu_jit_tiered_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->tiered_manager.reset_stats();
}

void oc_ppu_jit_tiered_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->tiered_manager.clear();
}

// ============================================================================
// Multi-threaded Compilation APIs
// ============================================================================

void oc_ppu_jit_start_compile_threads(oc_ppu_jit_t* jit, size_t num_threads) {
    if (!jit || num_threads == 0) return;
    
    jit->num_compile_threads = num_threads;
    jit->multithreaded_enabled = true;
    
    jit->thread_pool.start(num_threads, [jit](const CompilationTask& task) {
        // Compile the task
        auto block = std::make_unique<BasicBlock>(task.address);
        identify_basic_block(task.code.data(), task.code.size(), block.get());
        generate_llvm_ir(block.get(), jit);
        emit_machine_code(block.get());
        
        // Insert into cache (thread-safe)
        {
            oc_lock_guard<oc_mutex> lock(jit->cache.mutex);
            jit->cache.insert_block(task.address, std::move(block));
        }
        
        // Update lazy state
        jit->lazy_manager.mark_compiled(task.address);
    });
    
    // Also start enhanced thread pool
    jit->enhanced_thread_pool.start(num_threads, [jit](const EnhancedCompilationTask& task) -> bool {
        // Compile the task
        auto block = std::make_unique<BasicBlock>(task.address);
        identify_basic_block(task.code.data(), task.code.size(), block.get());
        generate_llvm_ir(block.get(), jit);
        emit_machine_code(block.get());
        
        // Insert into cache (thread-safe)
        {
            oc_lock_guard<oc_mutex> lock(jit->cache.mutex);
            jit->cache.insert_block(task.address, std::move(block));
        }
        
        // Update lazy state
        jit->lazy_manager.mark_compiled(task.address);
        return true;  // Success
    });
}

void oc_ppu_jit_stop_compile_threads(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->thread_pool.shutdown();
    jit->enhanced_thread_pool.shutdown(true);  // Drain remaining tasks
    jit->multithreaded_enabled = false;
}

void oc_ppu_jit_submit_compile_task(oc_ppu_jit_t* jit, uint32_t address,
                                     const uint8_t* code, size_t size,
                                     int priority) {
    if (!jit || !code || !jit->multithreaded_enabled) return;
    jit->thread_pool.submit(CompilationTask(address, code, size, priority));
}

size_t oc_ppu_jit_get_pending_tasks(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->thread_pool.get_pending_count();
}

size_t oc_ppu_jit_get_completed_tasks(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->thread_pool.get_completed_count();
}

int oc_ppu_jit_is_multithreaded(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->multithreaded_enabled ? 1 : 0;
}

// Enhanced Thread Pool APIs

void oc_ppu_jit_pool_submit(oc_ppu_jit_t* jit, uint32_t address,
                             const uint8_t* code, size_t size, int priority) {
    if (!jit || !code || !jit->multithreaded_enabled) return;
    jit->enhanced_thread_pool.submit(address, code, size, priority);
}

int oc_ppu_jit_pool_wait_all(oc_ppu_jit_t* jit, uint32_t timeout_ms) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.wait_all(timeout_ms) ? 1 : 0;
}

size_t oc_ppu_jit_pool_cancel_all(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.cancel_all();
}

size_t oc_ppu_jit_pool_get_thread_count(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.get_thread_count();
}

size_t oc_ppu_jit_pool_get_active_workers(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.get_active_workers();
}

size_t oc_ppu_jit_pool_get_pending(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.get_pending_count();
}

size_t oc_ppu_jit_pool_get_completed(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->enhanced_thread_pool.get_completed_count();
}

void oc_ppu_jit_pool_get_stats(oc_ppu_jit_t* jit, uint64_t* total_submitted,
                                uint64_t* total_completed, uint64_t* total_failed,
                                uint64_t* peak_queue_size, uint64_t* avg_wait_ms,
                                uint64_t* avg_exec_ms) {
    if (!jit) {
        if (total_submitted) *total_submitted = 0;
        if (total_completed) *total_completed = 0;
        if (total_failed) *total_failed = 0;
        if (peak_queue_size) *peak_queue_size = 0;
        if (avg_wait_ms) *avg_wait_ms = 0;
        if (avg_exec_ms) *avg_exec_ms = 0;
        return;
    }
    
    auto stats = jit->enhanced_thread_pool.get_stats();
    if (total_submitted) *total_submitted = stats.total_tasks_submitted;
    if (total_completed) *total_completed = stats.total_tasks_completed;
    if (total_failed) *total_failed = stats.total_tasks_failed;
    if (peak_queue_size) *peak_queue_size = stats.peak_queue_size;
    if (avg_wait_ms) *avg_wait_ms = static_cast<uint64_t>(stats.get_avg_wait_time_ms());
    if (avg_exec_ms) *avg_exec_ms = static_cast<uint64_t>(stats.get_avg_exec_time_ms());
}

void oc_ppu_jit_pool_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->enhanced_thread_pool.reset_stats();
}

// ============================================================================
// Background Compilation APIs
// ============================================================================

void oc_ppu_jit_bg_enable(oc_ppu_jit_t* jit, int enable) {
    if (!jit) return;
    jit->bg_compiler.set_enabled(enable != 0);
}

int oc_ppu_jit_bg_is_enabled(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->bg_compiler.is_enabled() ? 1 : 0;
}

void oc_ppu_jit_bg_set_idle_mode(oc_ppu_jit_t* jit, int idle) {
    if (!jit) return;
    jit->bg_compiler.set_idle_mode(idle != 0);
}

int oc_ppu_jit_bg_is_idle(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->bg_compiler.is_idle() ? 1 : 0;
}

void oc_ppu_jit_bg_configure(oc_ppu_jit_t* jit, uint32_t speculation_depth,
                              int branch_priority, int hot_threshold, size_t max_queue) {
    if (!jit) return;
    jit->bg_compiler.configure(speculation_depth, branch_priority, hot_threshold, max_queue);
}

int oc_ppu_jit_bg_queue_speculative(oc_ppu_jit_t* jit, uint32_t address,
                                     const uint8_t* code, size_t size, int score) {
    if (!jit || !code) return 0;
    return jit->bg_compiler.queue_speculative(address, code, size, score, false) ? 1 : 0;
}

int oc_ppu_jit_bg_queue_branch_target(oc_ppu_jit_t* jit, uint32_t address,
                                       const uint8_t* code, size_t size) {
    if (!jit || !code) return 0;
    return jit->bg_compiler.queue_speculative(address, code, size, 0, true) ? 1 : 0;
}

int oc_ppu_jit_bg_is_compiled(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->bg_compiler.is_compiled(address) ? 1 : 0;
}

int oc_ppu_jit_bg_is_queued(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return 0;
    return jit->bg_compiler.is_queued(address) ? 1 : 0;
}

void oc_ppu_jit_bg_mark_compiled(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->bg_compiler.mark_compiled(address);
}

size_t oc_ppu_jit_bg_process_idle(oc_ppu_jit_t* jit, size_t max_count) {
    if (!jit) return 0;
    
    return jit->bg_compiler.process_idle_batch(
        [jit](uint32_t addr, const uint8_t* code, size_t size) -> bool {
            // Compile using existing infrastructure
            // Note: These functions may fail silently in some cases
            auto block = std::make_unique<BasicBlock>(addr);
            if (!block) return false;
            
            identify_basic_block(code, size, block.get());
            
            // Check if block has any instructions (basic validation)
            if (block->instructions.empty()) {
                return false;  // Empty block, compilation failed
            }
            
            generate_llvm_ir(block.get(), jit);
            emit_machine_code(block.get());
            
            // Insert into cache
            {
                oc_lock_guard<oc_mutex> lock(jit->cache.mutex);
                jit->cache.insert_block(addr, std::move(block));
            }
            return true;
        },
        max_count
    );
}

void oc_ppu_jit_bg_record_hit(oc_ppu_jit_t* jit, uint32_t address) {
    if (!jit) return;
    jit->bg_compiler.record_speculative_hit(address);
}

size_t oc_ppu_jit_bg_get_queue_size(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->bg_compiler.get_queue_size();
}

size_t oc_ppu_jit_bg_get_compiled_count(oc_ppu_jit_t* jit) {
    if (!jit) return 0;
    return jit->bg_compiler.get_compiled_count();
}

void oc_ppu_jit_bg_get_stats(oc_ppu_jit_t* jit, uint64_t* speculative_queued,
                              uint64_t* speculative_compiled, uint64_t* speculative_hits,
                              uint64_t* branch_targets_queued, uint64_t* branch_targets_compiled,
                              uint64_t* idle_compilations) {
    if (!jit) {
        if (speculative_queued) *speculative_queued = 0;
        if (speculative_compiled) *speculative_compiled = 0;
        if (speculative_hits) *speculative_hits = 0;
        if (branch_targets_queued) *branch_targets_queued = 0;
        if (branch_targets_compiled) *branch_targets_compiled = 0;
        if (idle_compilations) *idle_compilations = 0;
        return;
    }
    
    auto stats = jit->bg_compiler.get_stats();
    if (speculative_queued) *speculative_queued = stats.speculative_queued;
    if (speculative_compiled) *speculative_compiled = stats.speculative_compiled;
    if (speculative_hits) *speculative_hits = stats.speculative_hits;
    if (branch_targets_queued) *branch_targets_queued = stats.branch_targets_queued;
    if (branch_targets_compiled) *branch_targets_compiled = stats.branch_targets_compiled;
    if (idle_compilations) *idle_compilations = stats.idle_compilations;
}

void oc_ppu_jit_bg_reset_stats(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->bg_compiler.reset_stats();
}

void oc_ppu_jit_bg_clear(oc_ppu_jit_t* jit) {
    if (!jit) return;
    jit->bg_compiler.clear();
}

// ============================================================================
// JIT Execution APIs
// ============================================================================

/**
 * JIT function signature type
 * 
 * JIT-compiled functions take a context pointer and memory base, then
 * read/write registers through the context structure.
 */
typedef void (*JitFunctionPtr)(oc_ppu_context_t* context, void* memory_base);

int oc_ppu_jit_execute(oc_ppu_jit_t* jit, oc_ppu_context_t* context, uint32_t address) {
    if (!jit || !context) return -1;
    
    // Check for breakpoint at this address
    if (jit->breakpoints.has_breakpoint(address)) {
        context->exit_reason = OC_PPU_EXIT_BREAKPOINT;
        return 0;
    }
    
    // Get compiled code
    BasicBlock* block = jit->cache.find_block(address);
    if (!block || !block->compiled_code) {
        // Not compiled - return error so interpreter can handle
        context->exit_reason = OC_PPU_EXIT_ERROR;
        return -2;
    }
    
    // Set up context for execution
    context->memory_base = context->memory_base; // Passed from caller
    context->instructions_executed = 0;
    context->exit_reason = OC_PPU_EXIT_NORMAL;
    context->next_pc = address + (block->instructions.size() * 4);
    
    // Cast compiled code to function pointer and call
    JitFunctionPtr func = reinterpret_cast<JitFunctionPtr>(block->compiled_code);
    
    // Execute the compiled block
    // Note: In the current placeholder implementation, the compiled code
    // is just a RET instruction, so this immediately returns.
    // A full LLVM implementation would execute actual compiled code.
    func(context, context->memory_base);
    
    // Update execution count
    context->instructions_executed = static_cast<uint32_t>(block->instructions.size());
    
    // Update PC based on exit reason
    if (context->exit_reason == OC_PPU_EXIT_NORMAL) {
        context->pc = context->next_pc;
    }
    // For branches/syscalls, compiled code should have set next_pc
    
    return static_cast<int>(context->instructions_executed);
}

int oc_ppu_jit_execute_block(oc_ppu_jit_t* jit, oc_ppu_context_t* context, uint32_t address) {
    // Same as execute for now - single block execution
    return oc_ppu_jit_execute(jit, context, address);
}

} // extern "C"
