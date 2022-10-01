# rust-debug

Provides a layer of abstraction over the `DWARF` debug format.

This crate makes it easier to get debug information from the `DWARF` debug format.
It also helps with retrieving information from the debug target, it requires
that another crate for interacting with the debug target.

## Features

* Preforming a stack trace.
* Virtually unwinding the call stack.
* Evaluating a variable.
* Finding a breakpoint location.
* Retrieving the source code location where a `DIE` was declared.

## Example

Check out the debugger `ERDB` that was made using this crate.
[https://github.com/Blinningjr/embedded-rust-debugger](https://github.com/Blinningjr/embedded-rust-debugger)

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

