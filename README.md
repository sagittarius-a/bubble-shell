# OSH - Opinionated Shell

OSH is based on [bubble-shell](https://github.com/JoshMcguigan/bubble-shell),
as a toy project to:

- Use Rust outside my consort zone
- Figure out how complex it is to build a featureful shell
- Build the perfect KISS shell for me, eventually. No plugins, no features that I will never use. Bare metal features only.

As the name suggest, OSH is opinionated and may not fulfil your expectations. That's okay.

## Features

- A single but classic prompt theme
- Completion (triggered with TAB) based on non-regex pattern. If several candidates are found, `skim` is used to filter them.
- Basic alias support
- Global alias support: Alias can be used after `|` character
- Environment variable expansion
- Configuration can be edited thanks to the builtin `config` command. It uses `$EDITOR` as editor to open configuration file
- Completion hints
- Some additional keybindings have been implemented, such as:
	- `CTRL + f`: Accept completion hint
	- `CTRL + o`: Enter
	- `ALT + f`: Go forward a word
	- `ALT + w`: Go backward a word
	- `ALT + u`: Undo (That is soooooo cool)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE.txt) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
