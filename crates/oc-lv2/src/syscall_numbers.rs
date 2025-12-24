//! PS3 LV2 syscall numbers

// Process management
pub const SYS_PROCESS_GETPID: u64 = 1;
pub const SYS_PROCESS_EXIT: u64 = 2;
pub const SYS_PROCESS_GETPPID: u64 = 3;
pub const SYS_PROCESS_GET_SDK_VERSION: u64 = 25;
pub const SYS_PROCESS_GET_STATUS: u64 = 22;
pub const SYS_PROCESS_GET_PARAMSFO: u64 = 26;
pub const SYS_GAME_PROCESS_EXITSPAWN: u64 = 363;

// PPU Thread management
pub const SYS_PPU_THREAD_CREATE: u64 = 160;
pub const SYS_PPU_THREAD_START: u64 = 53;
pub const SYS_PPU_THREAD_JOIN: u64 = 161;
pub const SYS_PPU_THREAD_DETACH: u64 = 51;
pub const SYS_PPU_THREAD_YIELD: u64 = 43;
pub const SYS_PPU_THREAD_GET_ID: u64 = 44;
pub const SYS_PPU_THREAD_EXIT: u64 = 41;
pub const SYS_PPU_THREAD_GET_PRIORITY: u64 = 49;
pub const SYS_PPU_THREAD_SET_PRIORITY: u64 = 50;

// Mutex
pub const SYS_MUTEX_CREATE: u64 = 100;
pub const SYS_MUTEX_DESTROY: u64 = 101;
pub const SYS_MUTEX_LOCK: u64 = 102;
pub const SYS_MUTEX_TRYLOCK: u64 = 103;
pub const SYS_MUTEX_UNLOCK: u64 = 104;

// Condition variable
pub const SYS_COND_CREATE: u64 = 105;
pub const SYS_COND_DESTROY: u64 = 106;
pub const SYS_COND_WAIT: u64 = 107;
pub const SYS_COND_SIGNAL: u64 = 108;
pub const SYS_COND_SIGNAL_ALL: u64 = 109;

// RwLock
pub const SYS_RWLOCK_CREATE: u64 = 110;
pub const SYS_RWLOCK_DESTROY: u64 = 111;
pub const SYS_RWLOCK_RLOCK: u64 = 112;
pub const SYS_RWLOCK_TRYRLOCK: u64 = 113;
pub const SYS_RWLOCK_WLOCK: u64 = 114;
pub const SYS_RWLOCK_TRYWLOCK: u64 = 115;
pub const SYS_RWLOCK_UNLOCK: u64 = 116;

// Semaphore
pub const SYS_SEMAPHORE_CREATE: u64 = 117;
pub const SYS_SEMAPHORE_DESTROY: u64 = 118;
pub const SYS_SEMAPHORE_WAIT: u64 = 119;
pub const SYS_SEMAPHORE_TRYWAIT: u64 = 120;
pub const SYS_SEMAPHORE_POST: u64 = 121;
pub const SYS_SEMAPHORE_GET_VALUE: u64 = 122;

// Time
pub const SYS_TIME_GET_SYSTEM_TIME: u64 = 145;
pub const SYS_TIME_GET_TIMEBASE_FREQUENCY: u64 = 147;
pub const SYS_TIME_GET_CURRENT_TIME: u64 = 145; // Alias for SYS_TIME_GET_SYSTEM_TIME
pub const SYS_TIME_USLEEP: u64 = 146;

// Event queue
pub const SYS_EVENT_QUEUE_CREATE: u64 = 128;
pub const SYS_EVENT_QUEUE_DESTROY: u64 = 129;
pub const SYS_EVENT_QUEUE_RECEIVE: u64 = 130;
pub const SYS_EVENT_QUEUE_TRYRECEIVE: u64 = 131;

// Event port
pub const SYS_EVENT_PORT_CREATE: u64 = 132;
pub const SYS_EVENT_PORT_DESTROY: u64 = 133;
pub const SYS_EVENT_PORT_SEND: u64 = 135;

// SPU thread group
pub const SYS_SPU_THREAD_GROUP_CREATE: u64 = 150;
pub const SYS_SPU_THREAD_GROUP_DESTROY: u64 = 151;
pub const SYS_SPU_THREAD_GROUP_START: u64 = 153;
pub const SYS_SPU_THREAD_GROUP_JOIN: u64 = 155;

// SPU thread
pub const SYS_SPU_THREAD_INITIALIZE: u64 = 169;
pub const SYS_SPU_IMAGE_OPEN: u64 = 156;
pub const SYS_SPU_THREAD_WRITE_LS: u64 = 171;
pub const SYS_SPU_THREAD_READ_LS: u64 = 172;

// Memory
pub const SYS_MEMORY_ALLOCATE: u64 = 324;
pub const SYS_MEMORY_FREE: u64 = 325;
pub const SYS_MEMORY_GET_PAGE_ATTRIBUTE: u64 = 327;
pub const SYS_MEMORY_GET_USER_MEMORY_SIZE: u64 = 352;
pub const SYS_MMAPPER_ALLOCATE_MEMORY: u64 = 330;
pub const SYS_MMAPPER_MAP_MEMORY: u64 = 331;

// File system
pub const SYS_FS_OPEN: u64 = 800;
pub const SYS_FS_CLOSE: u64 = 804;
pub const SYS_FS_READ: u64 = 801;
pub const SYS_FS_WRITE: u64 = 802;
pub const SYS_FS_LSEEK: u64 = 805;
pub const SYS_FS_FSTAT: u64 = 808;
pub const SYS_FS_STAT: u64 = 807;
pub const SYS_FS_OPENDIR: u64 = 811;
pub const SYS_FS_READDIR: u64 = 812;
pub const SYS_FS_CLOSEDIR: u64 = 814;

// TTY
pub const SYS_TTY_WRITE: u64 = 403;

// PRX module
pub const SYS_PRX_LOAD_MODULE: u64 = 451;
pub const SYS_PRX_START_MODULE: u64 = 452;
pub const SYS_PRX_STOP_MODULE: u64 = 453;
pub const SYS_PRX_UNLOAD_MODULE: u64 = 454;
pub const SYS_PRX_GET_MODULE_LIST: u64 = 461;
pub const SYS_PRX_GET_MODULE_INFO: u64 = 462;

