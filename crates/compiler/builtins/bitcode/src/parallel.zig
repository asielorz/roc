pub const TaskFn = *const fn (*anyopaque) callconv(.C) void;

pub extern fn roc_parallel_context_create(task_num_hint: usize) callconv(.C) *anyopaque;
pub extern fn roc_parallel_context_register_task(context: *anyopaque, task: TaskFn, params: *const anyopaque) callconv(.C) void;
pub extern fn roc_parallel_context_run(context: *anyopaque) callconv(.C) void;
pub extern fn roc_parallel_context_destroy(context: *anyopaque) callconv(.C) void;
