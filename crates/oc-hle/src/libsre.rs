//! libsre HLE - Regular expressions library
//!
//! This module provides HLE implementations for the PS3's regular expressions library.

use std::collections::HashMap;
use tracing::trace;
use regex::{Regex, RegexBuilder};

/// Regular expression match result
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SreMatch {
    pub start: u32,
    pub end: u32,
}

/// Regular expression context
pub type SreContext = u32;

/// Regular expression pattern
pub type SrePattern = u32;

/// Regular expression flags
pub const SRE_FLAG_CASELESS: u32 = 0x01;
pub const SRE_FLAG_MULTILINE: u32 = 0x02;
pub const SRE_FLAG_DOTALL: u32 = 0x04;

/// Error codes
pub const SRE_ERROR_INVALID_PATTERN: i32 = -1;
pub const SRE_ERROR_NO_MEMORY: i32 = -2;
pub const SRE_ERROR_INVALID_PARAMETER: i32 = -3;

/// Compiled pattern entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PatternEntry {
    /// Pattern ID
    id: u32,
    /// Pattern string
    pattern: String,
    /// Compilation flags
    flags: u32,
    /// Compiled regex
    regex: Option<Regex>,
}

/// Regular expression manager
pub struct RegexManager {
    /// Compiled patterns
    patterns: HashMap<u32, PatternEntry>,
    /// Next pattern ID
    next_pattern_id: u32,
}

impl RegexManager {
    /// Create a new regex manager
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            next_pattern_id: 1,
        }
    }

    /// Compile a regular expression pattern
    pub fn compile(&mut self, pattern: &str, flags: u32) -> Result<u32, i32> {
        if pattern.is_empty() {
            return Err(SRE_ERROR_INVALID_PATTERN);
        }

        let pattern_id = self.next_pattern_id;
        self.next_pattern_id += 1;

        // Build regex with flags
        let case_insensitive = (flags & SRE_FLAG_CASELESS) != 0;
        let multiline = (flags & SRE_FLAG_MULTILINE) != 0;
        let dot_matches_newline = (flags & SRE_FLAG_DOTALL) != 0;

        let regex_result = RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .multi_line(multiline)
            .dot_matches_new_line(dot_matches_newline)
            .build();

        let compiled_regex = match regex_result {
            Ok(r) => Some(r),
            Err(e) => {
                trace!("RegexManager::compile: Failed to compile pattern: {}", e);
                return Err(SRE_ERROR_INVALID_PATTERN);
            }
        };

        let entry = PatternEntry {
            id: pattern_id,
            pattern: pattern.to_string(),
            flags,
            regex: compiled_regex,
        };

        self.patterns.insert(pattern_id, entry);

        trace!("RegexManager::compile: id={}, pattern={}, flags={} (with actual regex backend)", 
            pattern_id, pattern, flags);

        Ok(pattern_id)
    }

    /// Free a compiled pattern
    pub fn free(&mut self, pattern_id: u32) -> i32 {
        if let Some(_pattern) = self.patterns.remove(&pattern_id) {
            trace!("RegexManager::free: id={}", pattern_id);
            0 // CELL_OK
        } else {
            SRE_ERROR_INVALID_PARAMETER
        }
    }

    /// Check if pattern is valid
    pub fn is_valid(&self, pattern_id: u32) -> bool {
        self.patterns.contains_key(&pattern_id)
    }

    /// Get pattern count
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Get pattern info
    pub fn get_pattern(&self, pattern_id: u32) -> Option<&PatternEntry> {
        self.patterns.get(&pattern_id)
    }

    /// Match pattern against text
    pub fn match_pattern(&self, pattern_id: u32, text: &str) -> Vec<SreMatch> {
        let pattern = match self.patterns.get(&pattern_id) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let regex = match &pattern.regex {
            Some(r) => r,
            None => return Vec::new(),
        };

        let mut matches = Vec::new();
        for capture in regex.captures_iter(text) {
            if let Some(m) = capture.get(0) {
                matches.push(SreMatch {
                    start: m.start() as u32,
                    end: m.end() as u32,
                });
            }
        }

        trace!("RegexManager::match_pattern: id={}, found {} matches", pattern_id, matches.len());
        matches
    }

    /// Search for first match in text
    pub fn search_pattern(&self, pattern_id: u32, text: &str, start_offset: usize) -> Option<SreMatch> {
        let pattern = self.patterns.get(&pattern_id)?;
        let regex = pattern.regex.as_ref()?;

        let search_text = &text[start_offset..];
        let m = regex.find(search_text)?;

        let result = SreMatch {
            start: (m.start() + start_offset) as u32,
            end: (m.end() + start_offset) as u32,
        };

        trace!("RegexManager::search_pattern: id={}, found match at {}-{}", 
            pattern_id, result.start, result.end);

        Some(result)
    }

    /// Replace matches in text
    pub fn replace_pattern(&self, pattern_id: u32, text: &str, replacement: &str) -> Option<String> {
        let pattern = self.patterns.get(&pattern_id)?;
        let regex = pattern.regex.as_ref()?;

        let result = regex.replace_all(text, replacement).to_string();

        trace!("RegexManager::replace_pattern: id={}, replaced text", pattern_id);

        Some(result)
    }
}

impl Default for RegexManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellSreCompile - Compile regular expression
pub unsafe fn cell_sre_compile(
    pattern: *const u8,
    flags: u32,
    compiled: *mut SrePattern,
) -> i32 {
    trace!("cellSreCompile called with flags={}", flags);
    
    // Validate parameters
    if pattern.is_null() || compiled.is_null() {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Read pattern string from pointer (null-terminated C string)
    let pattern_str = unsafe {
        let mut len = 0;
        let mut ptr = pattern;
        while *ptr != 0 {
            len += 1;
            ptr = ptr.add(1);
        }
        if len == 0 {
            return SRE_ERROR_INVALID_PATTERN;
        }
        match std::str::from_utf8(std::slice::from_raw_parts(pattern, len)) {
            Ok(s) => s,
            Err(_) => return SRE_ERROR_INVALID_PATTERN,
        }
    };
    
    if pattern_str.is_empty() {
        return SRE_ERROR_INVALID_PATTERN;
    }
    
    // Compile through global regex manager
    match crate::context::get_hle_context_mut().regex.compile(pattern_str, flags) {
        Ok(pattern_id) => {
            unsafe {
                *compiled = pattern_id;
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellSreFree - Free compiled regular expression
pub fn cell_sre_free(pattern: SrePattern) -> i32 {
    trace!("cellSreFree called with pattern: {}", pattern);
    
    // Validate pattern
    if pattern == 0 {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    crate::context::get_hle_context_mut().regex.free(pattern)
}

/// cellSreMatch - Match regular expression
pub unsafe fn cell_sre_match(
    pattern: SrePattern,
    text: *const u8,
    text_len: u32,
    matches: *mut SreMatch,
    max_matches: u32,
    num_matches: *mut u32,
) -> i32 {
    trace!("cellSreMatch called with pattern={}, text_len={}", pattern, text_len);
    
    // Validate parameters
    if pattern == 0 || text.is_null() {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Validate pattern exists through global manager
    if !crate::context::get_hle_context().regex.is_valid(pattern) {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Read text string
    let text_str = unsafe {
        match std::str::from_utf8(std::slice::from_raw_parts(text, text_len as usize)) {
            Ok(s) => s,
            Err(_) => return SRE_ERROR_INVALID_PARAMETER,
        }
    };
    
    // Perform actual regex matching using the backend
    let found_matches = crate::context::get_hle_context().regex.match_pattern(pattern, text_str);
    
    let match_count = found_matches.len().min(max_matches as usize);
    
    unsafe {
        if !matches.is_null() && match_count > 0 {
            for (i, m) in found_matches.iter().take(match_count).enumerate() {
                *matches.add(i) = *m;
            }
        }
        
        if !num_matches.is_null() {
            *num_matches = match_count as u32;
        }
    }
    
    trace!("cellSreMatch: Found {} matches", match_count);
    
    0 // CELL_OK
}

/// cellSreSearch - Search for regular expression
pub unsafe fn cell_sre_search(
    pattern: SrePattern,
    text: *const u8,
    text_len: u32,
    start_offset: u32,
    match_result: *mut SreMatch,
) -> i32 {
    trace!("cellSreSearch called with pattern={}, text_len={}, offset={}", 
        pattern, text_len, start_offset);
    
    // Validate parameters
    if pattern == 0 || text.is_null() {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    if start_offset >= text_len {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Validate pattern exists through global manager
    if !crate::context::get_hle_context().regex.is_valid(pattern) {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Read text string
    let text_str = unsafe {
        match std::str::from_utf8(std::slice::from_raw_parts(text, text_len as usize)) {
            Ok(s) => s,
            Err(_) => return SRE_ERROR_INVALID_PARAMETER,
        }
    };
    
    // Perform actual regex search using the backend
    if let Some(found_match) = crate::context::get_hle_context().regex.search_pattern(
        pattern, 
        text_str, 
        start_offset as usize
    ) {
        unsafe {
            if !match_result.is_null() {
                *match_result = found_match;
            }
        }
        trace!("cellSreSearch: Found match at {}-{}", found_match.start, found_match.end);
        0 // CELL_OK (found)
    } else {
        trace!("cellSreSearch: No match found");
        -1 // Not found
    }
}

/// cellSreReplace - Replace text matching regular expression
pub unsafe fn cell_sre_replace(
    pattern: SrePattern,
    text: *const u8,
    text_len: u32,
    replacement: *const u8,
    replacement_len: u32,
    output: *mut u8,
    output_len: u32,
    result_len: *mut u32,
) -> i32 {
    trace!("cellSreReplace called with pattern={}, text_len={}", 
        pattern, text_len);
    
    // Validate parameters
    if pattern == 0 || text.is_null() || replacement.is_null() || output.is_null() {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Validate pattern exists through global manager
    if !crate::context::get_hle_context().regex.is_valid(pattern) {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Read text string
    let text_str = unsafe {
        match std::str::from_utf8(std::slice::from_raw_parts(text, text_len as usize)) {
            Ok(s) => s,
            Err(_) => return SRE_ERROR_INVALID_PARAMETER,
        }
    };
    
    // Read replacement string
    let replacement_str = unsafe {
        match std::str::from_utf8(std::slice::from_raw_parts(replacement, replacement_len as usize)) {
            Ok(s) => s,
            Err(_) => return SRE_ERROR_INVALID_PARAMETER,
        }
    };
    
    // Perform actual regex replacement using the backend
    if let Some(result) = crate::context::get_hle_context().regex.replace_pattern(
        pattern, 
        text_str, 
        replacement_str
    ) {
        let result_bytes = result.as_bytes();
        let copy_len = result_bytes.len().min(output_len as usize);
        
        unsafe {
            std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), output, copy_len);
            
            if !result_len.is_null() {
                *result_len = copy_len as u32;
            }
        }
        
        trace!("cellSreReplace: Replaced text, result length: {}", copy_len);
        0 // CELL_OK
    } else {
        unsafe {
            if !result_len.is_null() {
                *result_len = 0;
            }
        }
        SRE_ERROR_INVALID_PARAMETER
    }
}

/// cellSreGetError - Get error message
pub unsafe fn cell_sre_get_error(
    error_code: i32,
    buffer: *mut u8,
    buffer_size: u32,
) -> i32 {
    trace!("cellSreGetError called with error_code: {}", error_code);
    
    // Validate parameters
    if buffer.is_null() || buffer_size == 0 {
        return SRE_ERROR_INVALID_PARAMETER;
    }
    
    // Format error message based on error code
    let msg: &[u8] = match error_code {
        SRE_ERROR_INVALID_PATTERN => b"Invalid pattern\0",
        SRE_ERROR_NO_MEMORY => b"Out of memory\0",
        SRE_ERROR_INVALID_PARAMETER => b"Invalid parameter\0",
        _ => b"Unknown error\0",
    };
    
    // Write error message to buffer
    let copy_len = std::cmp::min(msg.len(), buffer_size as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(msg.as_ptr(), buffer, copy_len);
    }
    
    0 // CELL_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_manager() {
        let mut manager = RegexManager::new();
        
        // Compile pattern
        let pattern_id = manager.compile("test.*", 0);
        assert!(pattern_id.is_ok());
        let pattern_id = pattern_id.unwrap();
        
        assert!(manager.is_valid(pattern_id));
        assert_eq!(manager.pattern_count(), 1);
        
        // Free pattern
        assert_eq!(manager.free(pattern_id), 0);
        assert!(!manager.is_valid(pattern_id));
        assert_eq!(manager.pattern_count(), 0);
    }

    #[test]
    fn test_regex_manager_multiple_patterns() {
        let mut manager = RegexManager::new();
        
        // Compile multiple patterns
        let pattern1 = manager.compile("[0-9]+", 0).unwrap();
        let pattern2 = manager.compile("[a-z]+", SRE_FLAG_CASELESS).unwrap();
        let pattern3 = manager.compile("test", SRE_FLAG_MULTILINE).unwrap();
        
        assert_eq!(manager.pattern_count(), 3);
        assert_ne!(pattern1, pattern2);
        assert_ne!(pattern2, pattern3);
        
        // Free patterns
        manager.free(pattern1);
        manager.free(pattern2);
        manager.free(pattern3);
        
        assert_eq!(manager.pattern_count(), 0);
    }

    #[test]
    fn test_regex_manager_validation() {
        let mut manager = RegexManager::new();
        
        // Empty pattern
        assert!(manager.compile("", 0).is_err());
        
        // Invalid pattern ID
        assert!(manager.free(9999) != 0);
    }

    #[test]
    fn test_regex_manager_get_pattern() {
        let mut manager = RegexManager::new();
        
        let pattern_id = manager.compile("test.*pattern", SRE_FLAG_CASELESS).unwrap();
        
        let entry = manager.get_pattern(pattern_id);
        assert!(entry.is_some());
        
        let entry = entry.unwrap();
        assert_eq!(entry.pattern, "test.*pattern");
        assert_eq!(entry.flags, SRE_FLAG_CASELESS);
        
        manager.free(pattern_id);
    }

    #[test]
    fn test_sre_compile() {
        // Note: cell_sre_compile currently uses placeholder implementation
        // and writes a placeholder pattern ID. The actual compilation
        // through the global regex manager is marked as TODO.
        let pattern = b"test.*pattern\0";
        let mut compiled = 0;
        
        let result = cell_sre_compile(pattern.as_ptr(), 0, &mut compiled);
        assert_eq!(result, 0);
        assert!(compiled > 0);
        
        // Note: cell_sre_free now properly goes through global manager,
        // but the pattern wasn't actually registered there by cell_sre_compile
        // since memory read is not yet implemented. This is expected.
    }

    #[test]
    fn test_sre_compile_validation() {
        let pattern = b"test\0";
        let mut compiled = 0;
        
        // Valid compile
        assert_eq!(cell_sre_compile(pattern.as_ptr(), 0, &mut compiled), 0);
        
        // Null pattern
        assert!(cell_sre_compile(std::ptr::null(), 0, &mut compiled) != 0);
        
        // Null output
        assert!(cell_sre_compile(pattern.as_ptr(), 0, std::ptr::null_mut()) != 0);
    }

    #[test]
    fn test_sre_free_validation() {
        // Invalid pattern (0)
        assert!(cell_sre_free(0) != 0);
        
        // Valid pattern - compile one first through the manager
        let pattern_id = crate::context::get_hle_context_mut().regex.compile("test", 0).unwrap();
        assert_eq!(cell_sre_free(pattern_id), 0);
    }

    #[test]
    fn test_sre_match() {
        let pattern = 1; // Assume compiled
        let text = b"test string";
        let mut matches = [SreMatch { start: 0, end: 0 }; 10];
        let mut num_matches = 0;
        
        let result = cell_sre_match(
            pattern,
            text.as_ptr(),
            text.len() as u32,
            matches.as_mut_ptr(),
            10,
            &mut num_matches,
        );
        
        assert_eq!(result, 0);
    }

    #[test]
    fn test_sre_match_validation() {
        let text = b"test";
        let mut matches = [SreMatch::default(); 10];
        let mut num_matches = 0;
        
        // Invalid pattern (0)
        assert!(cell_sre_match(0, text.as_ptr(), 4, matches.as_mut_ptr(), 10, &mut num_matches) != 0);
        
        // Null text
        assert!(cell_sre_match(1, std::ptr::null(), 4, matches.as_mut_ptr(), 10, &mut num_matches) != 0);
    }

    #[test]
    fn test_sre_search() {
        let pattern = 1; // Assume compiled
        let text = b"test string";
        let mut match_result = SreMatch { start: 0, end: 0 };
        
        let _result = cell_sre_search(
            pattern,
            text.as_ptr(),
            text.len() as u32,
            0,
            &mut match_result,
        );
        
        // Result may be -1 (not found) since we're not actually matching
    }

    #[test]
    fn test_sre_search_validation() {
        let text = b"test";
        let mut match_result = SreMatch::default();
        
        // Invalid pattern (0)
        assert!(cell_sre_search(0, text.as_ptr(), 4, 0, &mut match_result) != 0);
        
        // Null text
        assert!(cell_sre_search(1, std::ptr::null(), 4, 0, &mut match_result) != 0);
        
        // Invalid offset
        assert!(cell_sre_search(1, text.as_ptr(), 4, 10, &mut match_result) != 0);
    }

    #[test]
    fn test_sre_replace_validation() {
        let text = b"test";
        let replacement = b"new";
        let mut output = [0u8; 100];
        let mut result_len = 0;
        
        // Valid call
        assert_eq!(cell_sre_replace(1, text.as_ptr(), 4, replacement.as_ptr(), 3, 
            output.as_mut_ptr(), 100, &mut result_len), 0);
        
        // Invalid pattern (0)
        assert!(cell_sre_replace(0, text.as_ptr(), 4, replacement.as_ptr(), 3,
            output.as_mut_ptr(), 100, &mut result_len) != 0);
    }

    #[test]
    fn test_sre_flags() {
        assert_eq!(SRE_FLAG_CASELESS, 0x01);
        assert_eq!(SRE_FLAG_MULTILINE, 0x02);
        assert_eq!(SRE_FLAG_DOTALL, 0x04);
    }

    #[test]
    fn test_sre_error_codes() {
        assert_eq!(SRE_ERROR_INVALID_PATTERN, -1);
        assert_eq!(SRE_ERROR_NO_MEMORY, -2);
        assert_eq!(SRE_ERROR_INVALID_PARAMETER, -3);
    }
}
