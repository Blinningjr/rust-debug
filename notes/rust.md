Comand to see all the codegen options.
```rustc -C help```

Forum talking about location information being removed on higher optimization levels.
https://users.rust-lang.org/t/opt-level-2-removes-debug-symbols-needed-in-perf-profiling/16835/2

LLVM optimization passes 
https://llvm.org/docs/Passes.html

LLVM debugging information 
https://llvm.org/docs/SourceLevelDebugging.html

Location of where the inline threshold is set
https://github.com/rust-lang/rust/blob/1.29.0/src/librustc_codegen_llvm/back/write.rs#L2105-L2122

Good link for optimization information:
https://gist.github.com/jFransham/369a86eff00e5f280ed25121454acec1

