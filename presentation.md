# A design for parallelism in Roc

This is going to be long. I apologize in advance. Bear with me if you're interested.

## TLDR

An idea for how Roc could offer parallelism in the language and standard library without having to involve tasks and the platform, and links to a fork of the compiler implementing the idea.

## Introduction

In the last episode of Software Unscripted, _Making JITted Code Faster with Chris Neurnberger_, when talking about compiler optimization and compilers or virtual machines that optimize too aggresively and unpredictably, Feldman mentions how in principle Roc could try to automatically parallelize the code, because being a purely functional language the end result would always be correct and safe. However, he also mentions how this feature would make the Roc optimizer a very unpredictable and unstable target, and writing efficient code for it would be a matter of tweaking the implementation to trigger the optimizer's internal heuristics, and that would be a bad position to be in because these heuristics could change from a version of the compiler to the next, effectively invalidating all this effort.

I wholeheartedly agree with this position. However, the episode had me thinking about potential designs for parallelism in Roc. I really like the idea that in a pure functional language parallelism is effectively pure, and I think it's a pity that currently in Roc one has to resort to tasks and whatever the platform offers for parallelism. This makes the code cumbersome to parallelize, more so for libraries that try to be platform agnostic.

In contrast, I would also like to point out to Haskell's `par` function as an example of doing parallelism with pure functions. `par` in Haskell has the type `par :: a -> b -> b`. There is no need for the IO monad. This works in Haskell thanks to two things. First, the Haskell runtime comes with a thread pool for running code in parallel, and this API leverages that. Second, since Haskell is lazy an expression will not do work immediately, it will just create a thunk that will be evaluated as needed. It is then easy to send that thunk to another thread to evaluate it in parallel. In Roc we don't have any of these two things, so the design will have to consider alternatives.

With this in mind, I have spent the weeking hacking on the Roc compiler and the basic-cli platform in order to explore and implement a potential design for parallelism in Roc.

## Why

The main point of this exercise is not to adopt my changes into Roc. I am not making a merge request with these. I am trying to open a conversation about the topic and suggest a direction. Of course, the language team has the last word about whether they are interested even and about every last detail of the design. I just want to contribute my two cents.

While I mostly care about the general idea, I have decided to present a concrete design and to implement it because I think that it is easier to talk about a concrete design than about an abstract idea. I am sure that the design will end up being greatly improved by the collective conversation, but regardless it should be a better starting point than just text in the abstract. It is also cool that you can download the compiler and try it yourself.

## Goals

The goals I had for the design were the following:

- Sequential code can be parallelized without modifying function signatures.
- Parallel code is pure and semantically equivalent to sequential code.
- Code can be parallelized independently of the platform it is written from. Platform agnostic libraries can write parallel code easily.
- The programmer has control over what code is parallel and what code is sequential.
- The platform controls how work is scheduled and synchronized.
- A platform can opt out from offering parallelism (because it doesn't want to or because it can't).
- A platform can easily implement the protocol in terms of a library (like Rayon, OpenMP, PPL...).
- No complex data layouts are fixed in the ABI, because those are almost impossible to change afterwards. This is important for evolution.

## Description of the design

### Platform parallelism protocol

Roc currently requires platforms to implement some functions, such as `roc_alloc` and `roc_dealloc`. The compiler, standard library and all existing Roc code can assume that these functions exist and do what they are expected to do, regardless of the platform.

I have extended the set of functions that all platforms must provide by adding 4 new functions. These functions define the protocol between the Roc compiler and the platform that let the language and standard library implement parallel algorithms.

The functions are the following:

- `void* roc_parallel_context_create(size_t task_count)`
- `void roc_parallel_context_register_task(void* context, task_fn task, const void* closure, const void* parameter, void* return_address)`
- `void roc_parallel_context_run(void* context)`
- `void roc_parallel_context_destroy(void* context)`

Where `task_fn` is a function pointer to `void(const void* closure, const void* parameter, void* return_address)`.

`roc_parallel_context_create` marks the start of a parallel scope. All tasks registered into a parallel context will potentially run in parallel. `roc_parallel_context_register_task` registers a task but does not run it yet. `roc_parallel_context_run` runs all the tasks registered into the context, potentially in parallel, and doesn't return until all tasks are guaranteed to be complete. `roc_parallel_context_destroy` destroys the context freeing any necessary resources.

You can find my implementation of the protocol for the basic-cli platform in my GitHub. It is written as a very thin wrapper over Rayon: https://github.com/asielorz/roc-basic-cli/blob/main/src/src/parallel.rs

With these functions, we can implement a fork and join parallelism model where a bunch of tasks are registered, ran and joined.

A platform that doesn't want to have or cannot have parallel execution can implement the protocol to sequentially execute all the code in the caller thread in `roc_parallel_context_run`, which means a platform can very easily opt out from offering parallelism if it wants, without any effect on the semantics of any program or library ran on top of it. This design does not force any platform to have parallel execution, it just gives them the opportunity to do so.

### Parallel map

Roc then offers two primives, which I implemented as a new keyword and a new library function written in Zig.

`List.parallelMap : List a, (a -> b) -> List b` is a function semantically equivalent to `List.map`, except that every function call is potentially ran in parallel. Any `List.map` in any Roc program can be substituted by `List.parallelMap` without modifying the program's behavior. If used correctly, this will make the program faster.

We achieve this by using the protocol mentioned above. This is the relevant part of my implementation of `List.parallelMap` in Zig, which you can also find at https://github.com/asielorz/roc/blob/main/crates/compiler/builtins/bitcode/src/list.zig.

```zig
var context = parallel.roc_parallel_context_create(size);

while (i < size) : (i += 1) {
    const task: parallel.TaskFn = @ptrCast(caller);
    const function_object: *const anyopaque = @ptrCast(data);
    const param: *const anyopaque = @ptrCast(source_ptr + (i * old_element_width));
    const return_address: *anyopaque = @ptrCast(target_ptr + (i * new_element_width));

    parallel.roc_parallel_context_register_task(context, task, function_object, param, return_address);
}

parallel.roc_parallel_context_run(context);
parallel.roc_parallel_context_destroy(context);
```

You can download my compiler from GitHub and start writing parallel maps today!

### `par` keyword

Where `List.parallelMap` lets a program execute the same function several times in parallel, the `par` keyword lets a program execute heterogeneous work in parallel. `par` is followed by a tuple constructor expression, and constructs a tuple, potentially initializing each member in parallel. For example:

```Roc
(a, b, c) = par (
    expensive_computation_1 x y z,
    expensive_computation_2 x y z,
    expensive_computation_3 x y z,
)
```

If we remove the `par` keyword, the program stays semantically exactly the same. It is just constucting a tuple. The first one does it in parallel, through.

```Roc
# The same, but sequential.
(a, b, c) = (
    expensive_computation_1 x y z,
    expensive_computation_2 x y z,
    expensive_computation_3 x y z,
)
```

Other important things to notice are that not only we are calling a different function in each of the expressions of the tuple, the types of `a`, `b` and `c` need not be the same.

The mechanism behind it is also the same as in `List.parallelMap`. A parallel context is created for the expression, then the initializer expressions of each of the members of the tuple are registered as tasks, and the context is ran and destroyed.

Initially, I tried implementing `par` in the library instead of in the language, with a design based around an applicative functor, but it was not possible to make it work because of how currying based applicative functors impose a sequential evaluation order even if there is no data dependency between the parameters, so I went for the keyword approach.

The keyword is not fully implemented in my GitHub fork. The parser will accept it and the generated code will work correctly, but there is no parallelism. It is currently equivalent to a tuple. The IR generation was too daunting for a weekend project on a new codebase, but I may retake it in the future to complete the demo.

## Links to the code

- Modified compiler: https://github.com/asielorz/roc
- Modified basic-cli platform: https://github.com/asielorz/roc-basic-cli

## Future work

### Design improvement and nitpicking

Of course, all of the function and keyword names and exact parameters are up to discussion. So is any syntax change. The final design is up to Richard Feldman and the rest of the language team. These are just ideas that I am presenting.

The important point I am trying to make is that there exists a direction to have parallelism in Roc that is easy to use and easy to share among platforms while also giving the platforms ultimate control over what goes on, by treating parallel functions as pure and extening the interface a platform must offer. The exact details matter less to me than the general idea.

The design could also be completely disregarded and that is fine too. It was a fun weekend project.

I think that the protocol can probably be improved to reduce overhead. For example, the separation between registering tasks and running them usually forces the platform to allocate a dynamic buffer to store the tasks, which could be avoided if the platform could start tasks as they are registered and `roc_parallel_context_run` could then just be a joining point.

It may also be possible to simplify it. For example, currently all uses of the protocol call `roc_parallel_context_run` and `roc_parallel_context_destroy` one after the other, so maybe they could be merged into a single function.

### Implementation improvement

As I said before, please do not merge my code. This is my first time working on the Roc codebase. I don't know what I am doing. Some parts are also intentionally quick and dirty, for simplicity, like the extra unnecessary allocation in my implementation of `roc_parallel_context_create`.

Besides, I don't know Zig, and I was mostly throwing code at the compiler until it worked, so my parallel map implementation can most likely be improved by someone who knows their thing.

### A bigger library

While the primitives are what Roc must offer, Roc doesn't necessarily only need to offer the primitives. It would be cool to have more parallel algorithms built on top of these primitives, such as parallel quicksort, parallel reduction and map-reduce... Maybe this could be an external library instead of part of the standard library too, but that is also a conversation to have.

## Conclusion

Thanks for reading all this, and sorry for the length of the post. I am really interested in criticism and alternative takes, so if you have ideas about how Roc should approach parallelism, please write them down. If you think the design presented could be changed or improved, that is also something I will be willing to read.

I am also curious about what the language team members think about the topic.
