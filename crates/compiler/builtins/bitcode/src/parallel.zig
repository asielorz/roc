pub const TaskFn = *const fn (*const anyopaque, *const anyopaque, *anyopaque) callconv(.C) void;

pub extern fn roc_parallel_context_create(task_num_hint: usize) callconv(.C) *anyopaque;
pub extern fn roc_parallel_context_register_task(context: *anyopaque, task: TaskFn, function_object: *const anyopaque, param: *const anyopaque, return_address: *anyopaque) callconv(.C) void;
pub extern fn roc_parallel_context_run(context: *anyopaque) callconv(.C) void;
pub extern fn roc_parallel_context_destroy(context: *anyopaque) callconv(.C) void;
