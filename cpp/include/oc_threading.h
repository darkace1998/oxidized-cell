/**
 * Cross-platform threading primitives for oxidized-cell
 * 
 * This header provides Windows-native threading implementations when using MinGW
 * to avoid linking issues with std::mutex and std::thread on the win32 threading model.
 * On other platforms (or when using POSIX threading), it falls back to the standard library.
 */

#ifndef OC_THREADING_H
#define OC_THREADING_H

#include <functional>
#include <atomic>

// Platform-specific threading for cross-compilation compatibility
// When using MinGW with win32 threading model, std::mutex may not work properly
// so we provide Windows-native implementations
#if defined(_WIN32) || defined(__MINGW32__)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

// Simple mutex wrapper using Windows CRITICAL_SECTION
class oc_mutex {
private:
    CRITICAL_SECTION cs;
public:
    oc_mutex() { InitializeCriticalSection(&cs); }
    ~oc_mutex() { DeleteCriticalSection(&cs); }
    void lock() { EnterCriticalSection(&cs); }
    void unlock() { LeaveCriticalSection(&cs); }
    CRITICAL_SECTION* native_handle() { return &cs; }
    oc_mutex(const oc_mutex&) = delete;
    oc_mutex& operator=(const oc_mutex&) = delete;
};

// RAII lock guard
template<typename Mutex>
class oc_lock_guard {
private:
    Mutex& mtx;
public:
    explicit oc_lock_guard(Mutex& m) : mtx(m) { mtx.lock(); }
    ~oc_lock_guard() { mtx.unlock(); }
    oc_lock_guard(const oc_lock_guard&) = delete;
    oc_lock_guard& operator=(const oc_lock_guard&) = delete;
};

// Unique lock (similar to std::unique_lock) with unlock capability
template<typename Mutex>
class oc_unique_lock {
private:
    Mutex& mtx;
    bool owns;
public:
    explicit oc_unique_lock(Mutex& m) : mtx(m), owns(true) { mtx.lock(); }
    ~oc_unique_lock() { if (owns) mtx.unlock(); }
    void unlock() { mtx.unlock(); owns = false; }
    void lock() { mtx.lock(); owns = true; }
    Mutex& mutex() { return mtx; }
    oc_unique_lock(const oc_unique_lock&) = delete;
    oc_unique_lock& operator=(const oc_unique_lock&) = delete;
};

// Condition variable using Windows primitives
class oc_condition_variable {
private:
    CONDITION_VARIABLE cv;
public:
    oc_condition_variable() { InitializeConditionVariable(&cv); }
    
    void notify_one() { WakeConditionVariable(&cv); }
    void notify_all() { WakeAllConditionVariable(&cv); }
    
    void wait(oc_unique_lock<oc_mutex>& lock) {
        SleepConditionVariableCS(&cv, lock.mutex().native_handle(), INFINITE);
    }
    
    template<typename Pred>
    void wait(oc_unique_lock<oc_mutex>& lock, Pred pred) {
        while (!pred()) {
            SleepConditionVariableCS(&cv, lock.mutex().native_handle(), INFINITE);
        }
    }
};

// Thread wrapper
class oc_thread {
private:
    HANDLE handle;
    static DWORD WINAPI thread_func(LPVOID arg) {
        auto* func = static_cast<std::function<void()>*>(arg);
        (*func)();
        delete func;
        return 0;
    }
public:
    oc_thread() : handle(NULL) {}
    
    template<typename F>
    explicit oc_thread(F&& f) {
        auto* func = new std::function<void()>(std::forward<F>(f));
        handle = CreateThread(NULL, 0, thread_func, func, 0, NULL);
    }
    
    bool joinable() const { return handle != NULL; }
    
    void join() {
        if (handle) {
            WaitForSingleObject(handle, INFINITE);
            CloseHandle(handle);
            handle = NULL;
        }
    }
    
    ~oc_thread() {
        if (handle) {
            CloseHandle(handle);
        }
    }
    
    oc_thread(oc_thread&& other) noexcept : handle(other.handle) {
        other.handle = NULL;
    }
    
    oc_thread& operator=(oc_thread&& other) noexcept {
        if (this != &other) {
            if (handle) CloseHandle(handle);
            handle = other.handle;
            other.handle = NULL;
        }
        return *this;
    }
    
    oc_thread(const oc_thread&) = delete;
    oc_thread& operator=(const oc_thread&) = delete;
};

#else
// On non-Windows platforms, use standard library
#include <mutex>
#include <condition_variable>
#include <thread>

using oc_mutex = std::mutex;
template<typename T>
using oc_lock_guard = std::lock_guard<T>;
template<typename T>
using oc_unique_lock = std::unique_lock<T>;
using oc_condition_variable = std::condition_variable;
using oc_thread = std::thread;
#endif

#endif // OC_THREADING_H
