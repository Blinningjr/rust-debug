# rust-debug
Is a library for retrieving debug information from the `DWARF` format.


The library provides a abstraction over the `DWARF` debug format, which simplifies the process of retrieving information from `DWARF`.


The goals for the library are:
* Easy to use.
* Provide one function solutions.
* Does not enforce which library is used to read from the debugged target.
* Does not restrict usage of `gimli-rs`.


## Features
* Preforming a stack trace.
* Virtually unwinding the call stack.
* Evaluating a variable.
* Finding a breakpoint location.
* Retrieving the source code location where a `DIE` was declared.


## Example
Check out this debugger for embedded Rust code I made using this library.
[https://github.com/Blinningjr/embedded-rust-debugger](https://github.com/Blinningjr/embedded-rust-debugger)


## Requirements
* All the provided function require something from `gimli-rs`
* Some of the function require that the debugged target registers and memory can be read.
* Some knowledge of the `DWARF` format


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

