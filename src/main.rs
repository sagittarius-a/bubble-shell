use console::style;

use std::borrow::Cow::{self, Borrowed, Owned};
use std::env::current_dir;
use std::fs;
use std::path::{self, Path};

use rustyline::completion::{escape, extract_word, unescape, Completer, Pair, Quote};
use rustyline::config::OutputStreamType;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{self, MatchingBracketValidator, Validator};
use rustyline::Movement;
use rustyline::Word;
use rustyline::{Cmd, CompletionType, Config, Context, EditMode, Editor, KeyEvent};
use rustyline_derive::Helper;

use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{stdout, Read, Write};
use std::process::{Child, Command, Stdio};

use log::LevelFilter;
use log::{debug, error, info, warn};
use log4rs::append::file::FileAppender;
use log4rs::config::Config as LogConfig;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;

extern crate skim;
use skim::prelude::*;
use std::io::Cursor;

extern crate shell_words;

macro_rules! wdebug {
    ($config:ident) => {
        if $config.debug {
            print!("\n")
        }
    };
    ($config:ident, $fmt:expr) => {
        if $config.debug {
            print!(concat!($fmt, "\n"));
            debug!($fmt);
        }
    };
    ($config:ident, $fmt:expr, $($arg:tt)*) => {
            if $config.debug {
            print!(concat!($fmt, "\n"), $($arg)*);
            debug!($fmt, $($arg)*);
        }
    };
}

macro_rules! werror {
    ($fmt:expr) => {
        eprint!(concat!($fmt, "\n"));
        error!($fmt);
    };
    ($fmt:expr, $($arg:tt)*) => {
        eprint!(concat!($fmt, "\n"), $($arg)*);
        error!($fmt, $($arg)*);
    };
}

macro_rules! winfo {
    ($fmt:expr) => {
        print!(concat!($fmt, "\n"));
        info!($fmt);
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\n"), $($arg)*);
        info!($fmt, $($arg)*);
    };
}

macro_rules! wwarning {
    ($fmt:expr) => {
        print!(concat!("Warning: ", $fmt, "\n"));
        warn!($fmt);
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!("Warning: ", $fmt, "\n"), $($arg)*);
        warn!($fmt, $($arg)*);
    };
}

#[derive(Helper)]
struct MyHelper {
    completer: MyFilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl Completer for MyHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Hinter for MyHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    /// Hint for command suggestions based on history
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
        // Owned("\x1b[31m".to_owned() + hint + "\x1b[m")
        Owned("\x1b[2m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        self.highlighter.highlight_char(line, pos)
    }
}

impl Validator for MyHelper {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<validate::ValidationResult> {
        self.validator.validate(ctx)
    }

    fn validate_while_typing(&self) -> bool {
        self.validator.validate_while_typing()
    }
}

/// A `Completer` for file and folder names.
pub struct MyFilenameCompleter {
    break_chars: &'static [u8],
    #[allow(dead_code)]
    double_quotes_special_chars: &'static [u8],
}

impl MyFilenameCompleter {
    pub fn new() -> Self {
        // Reuse values defined in rustyline
        const DEFAULT_BREAK_CHARS: [u8; 18] = [
            b' ', b'\t', b'\n', b'"', b'\\', b'\'', b'`', b'@', b'$', b'>', b'<', b'=', b';', b'|',
            b'&', b'{', b'(', b'\0',
        ];
        #[allow(dead_code)]
        const ESCAPE_CHAR: Option<char> = Some('\\');
        const DOUBLE_QUOTES_SPECIAL_CHARS: [u8; 4] = [b'"', b'$', b'\\', b'`'];

        Self {
            break_chars: &DEFAULT_BREAK_CHARS,
            double_quotes_special_chars: &DOUBLE_QUOTES_SPECIAL_CHARS,
        }
    }

    /// Some kind of fuzzy matching. Ues the last token as pattern and try to find it in files
    /// and directories in current directory
    /// Example: pattern `txt` would match any file containing it in name, such as `files.txt`
    /// or `txt-dir/`.
    pub fn try_complete_with_pattern(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // const ESCAPE_CHAR: Option<char> = Some('\\');
        const ESCAPE_CHAR: Option<char> = Some('\\');
        let (start, pattern) = extract_word(line, pos, ESCAPE_CHAR, self.break_chars);
        let pattern = unescape(pattern, ESCAPE_CHAR);

        let mut matches = self.complete_filename_with_pattern(
            &pattern,
            ESCAPE_CHAR,
            self.break_chars,
            Quote::None,
        );

        #[allow(clippy::unnecessary_sort_by)]
        matches.sort_by(|a, b| a.display.cmp(&b.display));
        Ok((start, matches))
    }

    fn complete_filename_with_pattern(
        &self,
        pattern: &str,
        esc_char: Option<char>,
        break_chars: &[u8],
        quote: Quote,
    ) -> Vec<Pair> {
        // Normalize directory and associated information
        let sep = path::MAIN_SEPARATOR;
        let (dir_name, file_name) = match pattern.rfind(sep) {
            Some(idx) => pattern.split_at(idx + sep.len_utf8()),
            None => ("", pattern),
        };

        let dir_path = Path::new(dir_name);
        let dir = if dir_path.is_relative() {
            if let Ok(cwd) = current_dir() {
                cwd.join(dir_path)
            } else {
                dir_path.to_path_buf()
            }
        } else {
            dir_path.to_path_buf()
        };

        let mut entries: Vec<Pair> = Vec::new();

        // if dir doesn't exist, then don't offer any completions
        if !dir.exists() {
            return entries;
        }

        let mut candidates = Vec::new();

        // Handle special patterns
        if pattern.contains("**") {
            candidates = self.list_all_files_and_directories(&dir);
        } else {
            // if any of the below IO operations have errors, just ignore them
            if let Ok(read_dir) = dir.read_dir() {
                for entry in read_dir.flatten() {
                    if let Some(s) = entry.file_name().to_str() {
                        if entry.file_name().to_str().unwrap().contains(file_name) {
                            if let Ok(_metadata) = fs::metadata(entry.path()) {
                                let candidate = String::from(dir_name) + s;
                                candidates.push(candidate);
                            }
                        }
                    }
                }
            }
        }

        if candidates.is_empty() {
            return entries;
        }

        if candidates.len() > 1 {
            // If multiple matches have been found, use skim to filter them
            let options = SkimOptionsBuilder::default()
                .height(Some("30%"))
                .multi(true)
                .reverse(true)
                .build()
                .unwrap();
            let item_reader = SkimItemReader::default();
            let items = item_reader.of_bufread(Cursor::new(candidates.join("\n")));

            #[allow(clippy::redundant_closure)]
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_else(|| Vec::new());

            let mut replacement = Vec::new();
            for item in selected_items.iter() {
                let name = escape(item.output().to_string(), esc_char, break_chars, quote);
                // Make sure to compute proper path by prefixing matched valued with the possible
                // directory they are in
                replacement.push(format!("{}{}", dir_name, name));
            }

            entries.push(Pair {
                display: "".into(), // No display needed since values will be changed in place
                replacement: replacement.join(" "),
            });
        } else {
            // If only one matching file has been found, return a single substitution so it will
            // be applied right away
            let candidate = candidates.first().unwrap().to_string();
            // Make sure to compute proper path by prefixing matched valued with the possible
            // directory they are in
            let path = format!("{}{}", dir_name, candidate);
            entries.push(Pair {
                display: "".into(), // No display needed since values will be changed in place
                replacement: escape(path, esc_char, break_chars, quote),
            });
        }

        entries
    }

    /// List all entries, both files and directories present in `dir`.
    fn list_all_files_and_directories(&self, dir: &Path) -> Vec<String> {
        let mut files = Vec::new();
        if let Ok(read_dir) = dir.read_dir() {
            // let file_name = self.normalize(file_name);
            for entry in read_dir.flatten() {
                files.push(entry.file_name().to_str().unwrap().to_string());
            }
        }
        files
    }
}

impl Default for MyFilenameCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl Completer for MyFilenameCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Provide custom completer that tries to complete filename based on a pattern.
        // If no pattern is provided, it will simply list files and directories.
        let (start, matches) = self.try_complete_with_pattern(line, pos, ctx).unwrap();

        Ok((start, matches))
    }
}

#[derive(Debug)]
struct ShellCommand {
    command: String,
    args: Vec<String>,
    redirection: Redirection,
    piped: bool,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
enum Redirection {
    None,
    Stdout,
    Stderr,
    Both,
}

fn build_commands(words: Vec<String>) -> Vec<ShellCommand> {
    let mut commands = Vec::new();

    // Find command separators
    let mut parts = Vec::new();
    let mut current = Vec::new();
    for w in words {
        if w.eq("|") {
            parts.push((current, true));
            current = Vec::new();
        } else {
            current.push(w)
        }
    }
    parts.push((current, false));

    for part in parts {
        if part.0.len() > 1 {
            let command = &part.0[0];
            let piped = &part.1;
            let redirection = Redirection::None;
            let args = &part.0[1..];

            // TODO: Parse args to find eventual redirection
            //
            // In order to perform this operation:
            // $ id > /tmp/asdf | grep uid
            //
            // This is not the kind of operation I'd like to do everyday but keeping a note is the
            // best way to think about it some day

            commands.push(ShellCommand {
                command: command.to_string(),
                args: args.to_vec(),
                redirection,
                piped: *piped,
            })
        } else {
            // Command only contains the command itself, no redirection, no pipe
            commands.push(ShellCommand {
                command: part.0[0].to_string(),
                args: Vec::new(),
                redirection: Redirection::None,
                piped: false,
            });
        }
    }

    commands
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    aliases: HashMap<String, String>,
    #[serde(default = "default_prompt")]
    prompt: String,
    #[serde(default)]
    debug: bool,
    #[serde(default = "get_username")]
    username: String,
    #[serde(default = "get_hostname")]
    hostname: String,
}

fn default_prompt() -> String {
    "$".to_string()
}

impl ConfigFile {
    fn new() -> ConfigFile {
        let mut config_file: Option<ConfigFile> = None;

        // Try to read the configuration file
        match std::fs::File::open(perform_expansion_on_single_element("~/.shell.yaml")) {
            Ok(f) => {
                // Load aliases
                config_file = serde_yaml::from_reader(f).unwrap();
            }
            Err(_) => {
                werror!("Cannot open configuration file '~/.shell.yaml'");
            }
        };
        match config_file {
            Some(c) => {
                wdebug!(c, "Config file: {:#?}", c);
                c
            }
            None => ConfigFile {
                aliases: HashMap::new(),
                prompt: default_prompt(),
                debug: false,
                username: get_username(),
                hostname: get_hostname(),
            },
        }
    }
}

#[allow(dead_code)]
/// Redirect stdout and/or stderr to a file
fn redirect(command: &str, output: &std::process::Output) {
    let filename;
    let mut file_options = OpenOptions::new();

    // Set default permissions
    file_options.write(true);

    let mut content = String::from_utf8_lossy(&output.stdout);

    let mut parts = command.split('>');

    // Determine if stderr needs to be redirected as well
    if parts.next().unwrap().ends_with('2') {
        let stderr = String::from_utf8_lossy(&output.stderr);
        content += stderr;
    }

    // Determine if the file must be truncated or if the content should be
    // appended
    if command.contains(">>") {
        file_options.append(true);
        filename = command.split(">>").nth(1).unwrap().trim();
    } else {
        file_options.truncate(true);
        file_options.create(true);
        filename = command.split('>').nth(1).unwrap().trim();
    }

    let mut file = file_options
        .open(filename)
        .unwrap_or_else(|_| panic!("Failed to open {} to redirect content to it", filename));
    write!(file, "{}", content)
        .unwrap_or_else(|_| panic!("Failed to write content to redirect to {}", filename));
}

fn setup_logging() {
    // https://docs.rs/log4rs/1.0.0/log4rs/encode/pattern/index.html
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} :: {l} - {m}\n",
        )))
        .build("/tmp/shell.log")
        .unwrap();

    let config = LogConfig::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))
        .unwrap();

    log4rs::init_config(config).unwrap();
}

/// Replace the `command` with an alias if available.
fn lookup_aliases(config: &ConfigFile, value: &str) -> Option<String> {
    config.aliases.get(value).map(|s| s.to_string())
}

// TODO: List aliases for better readability
fn list_aliases(aliases: &HashMap<String, String>) {
    for (key, value) in aliases.iter() {
        println!("{}: {}", key, value);
    }
}

/// Perform environment variable expansion.
fn perform_expansion_on_single_element(value: &str) -> String {
    // Expand tilde character
    if !value.contains('$') {
        if value.contains('~') && env::var("HOME").is_ok() {
            return value.replace('~', &env::var("HOME").unwrap());
        }
        return value.into();
    }

    // Replace environment variable
    let mut result = String::new();
    if let Some(key) = value.strip_prefix('$') {
        // Lookup for the given value in the environment
        result = match env::var(key) {
            Ok(x) => x,
            Err(_) => "".into(),
        };
    }

    result
}

fn get_username() -> String {
    let mut username = String::new();
    if let Ok(u) = env::var("USERNAME") {
        username = u;
    };
    username
}

fn get_hostname() -> String {
    let mut hostname = String::new();
    if let Ok(mut f) = std::fs::File::open("/etc/hostname") {
        let mut tmp = String::new();
        f.read_to_string(&mut tmp).unwrap();
        hostname = tmp.trim().into();
    };
    hostname
}

// Prompt format: user@host pwd
//                green     blue or red if status != 0
fn build_prompt(config: &ConfigFile, status: u32) -> String {
    let mut prompt = String::new();

    // Fetch current directory
    let cwd = match env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            werror!("{}", e);
            // Use empty value if current directory cannot be read from env
            // It is very unlikely but who knows
            std::path::PathBuf::new()
        }
    };

    wdebug!(config, "cwd: {:?}", cwd);
    wdebug!(config, "config.username: {:?}", config.username);

    if !config.username.is_empty() {
        prompt += &style(&config.username).green().to_string();
    }
    if !config.hostname.is_empty() {
        prompt += &style(format!("@{} ", &config.hostname)).green().to_string();
    }

    if status == 0 {
        prompt += &format!(
            "{} {} ",
            style(cwd.to_str().unwrap().replace("\"", ""))
                .blue()
                .bold()
                .to_string(),
            config.prompt
        );
    } else {
        prompt += &format!(
            "{} {} ",
            style(cwd.to_str().unwrap().replace("\"", ""))
                .red()
                .bold()
                .to_string(),
            config.prompt
        );
    }

    prompt
}

fn main() -> rustyline::Result<()> {
    setup_logging();

    // Initialize interactive prompt
    let mut previous_directory = env::current_dir().unwrap();

    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .tab_stop(4)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout)
        .build();

    let h = MyHelper {
        completer: MyFilenameCompleter::new(),
        highlighter: MatchingBracketHighlighter::new(),
        hinter: HistoryHinter {},
        colored_prompt: ">>>".to_owned(),
        validator: MatchingBracketValidator::new(),
    };

    // let mut rl = Editor::<()>::new();
    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(h));

    // Add custom keybindings to provide better user experience
    // It basically mimics features provided by various shell plugins
    rl.bind_sequence(KeyEvent::alt('n'), Cmd::HistorySearchForward);
    rl.bind_sequence(KeyEvent::alt('p'), Cmd::HistorySearchBackward);
    rl.bind_sequence(
        KeyEvent::alt('w'),
        Cmd::Kill(Movement::BackwardWord(1, Word::Emacs)),
    );
    rl.bind_sequence(KeyEvent::alt('u'), Cmd::Undo(1));
    rl.bind_sequence(KeyEvent::ctrl('f'), Cmd::CompleteHint);
    rl.bind_sequence(
        KeyEvent::ctrl('o'),
        Cmd::AcceptOrInsertLine {
            accept_in_the_middle: true,
        },
    );

    let homedir = match env::var("HOME") {
        Ok(val) => val,
        // Use /tmp as default directory if no $HOME directory has been found
        // This way, the user can still use this feature, even if the history
        // content won't survive reboots
        // TODO: Avoid collision when used by multiple users (even without $USER)
        Err(_) => "/tmp".into(),
    };
    let history = homedir + "/.history";
    let _ = rl.load_history(&history);
    let mut config = ConfigFile::new();
    let mut status = 0u32;

    'shell: loop {
        let prompt = build_prompt(&config, status);
        // Need to explicitly flush to ensure it prints before read_line
        stdout().flush().unwrap();
        rl.helper_mut().expect("No helper").colored_prompt = prompt.clone();

        match rl.readline(&prompt) {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }

                // Save input in history
                rl.add_history_entry(line.as_str());
                rl.save_history(&history).unwrap();

                let commands = shell_words::split(&line).expect("Failed to split command line");

                let mut previous_command = None;

                // For each command, use an alias if available. It allows user to use aliases
                // even in the commands following | character
                let mut resolved = Vec::new();
                for command in commands {
                    let expanded = perform_expansion_on_single_element(&command);
                    resolved.push(expanded);
                }

                // Now the command line has been preprocessed, split it in several commands to
                // execute
                let shell_commands = build_commands(resolved);

                for mut shell_command in shell_commands {
                    let mut command = shell_command.command;

                    // Handle alias related commands first
                    match &command[..] {
                        "alias" => {
                            // Register a new alias
                            let mut args = shell_command.args.iter();
                            let new_alias = match args.next() {
                                Some(v) => v.clone(),
                                None => {
                                    list_aliases(&config.aliases);
                                    status = 1;
                                    continue 'shell;
                                }
                            };

                            // Build the command by parsing the rest of the command provided
                            let aliased = args.cloned().collect::<Vec<String>>().join(" ");

                            config.aliases.insert(new_alias, aliased);
                            status = 0;
                        }

                        "unalias" => {
                            // Fetch the name of the new alias or display available aliases if not alias
                            // has been found
                            let mut args = shell_command.args.iter();
                            let request = match args.next() {
                                Some(v) => v,
                                None => {
                                    werror!("No alias provided");
                                    status = 1;
                                    continue 'shell;
                                }
                            };

                            if !config.aliases.contains_key(request) {
                                werror!("{} is not an alias", request);
                                status = 1;
                                continue 'shell;
                            }
                            config.aliases.remove(request);
                        }
                        _ => (),
                    }

                    // Now we're sure the command do not deal with alias, we can perform global
                    // alias resolving pretty safely
                    // Some nasty things could happen if the alias contains space separated value,
                    // but this is the user's responsibility
                    if let Some(aliased) = lookup_aliases(&config, &command) {
                        let aliased_parts = aliased
                            .split(' ')
                            .map(|a| a.to_string())
                            .collect::<Vec<String>>();
                        // If the alias contained space separated values, treat it as a command
                        // with arguments, that will be prepended to the original list of arguments
                        if aliased_parts.len() > 1 {
                            let mut new_args = Vec::new();
                            new_args.extend_from_slice(&aliased_parts[1..]);
                            new_args.extend_from_slice(&shell_command.args);
                            shell_command.args = new_args;
                        }
                        // Update command to execute
                        command = aliased_parts[0].to_string();
                    }

                    match &command[..] {
                        "config" => {
                            let editor = match env::var("EDITOR") {
                                Ok(e) => e,
                                Err(_) => {
                                    werror!(
                                        "EDITOR variable not set. Cannot open configuration file"
                                    );
                                    status = 1;
                                    continue 'shell;
                                }
                            };

                            let _ = Command::new(editor)
                                .args(vec![perform_expansion_on_single_element("~/.shell.yaml")])
                                .stdin(Stdio::inherit())
                                .stdout(Stdio::inherit())
                                .spawn()
                                .unwrap()
                                .wait();

                            status = 0;
                        }
                        "reload" => {
                            config = ConfigFile::new();
                            winfo!("Configuration file reloaded");
                            status = 0;
                        }
                        "status" => {
                            winfo!("Status: {}", status);
                            status = 0;
                        }
                        "cd" => {
                            // default to '~' of '/' as new directory if one was not provided
                            let dir = match env::var("HOME") {
                                Ok(val) => val,
                                Err(_) => {
                                    werror!("HOME variable not set. Using / as default target");
                                    "/".into()
                                }
                            };
                            let mut args = shell_command.args.iter();
                            let new_dir = match args.next() {
                                Some(v) => v,
                                None => &dir[..],
                            };

                            let target;
                            // Use "-" to go to the last directory visited
                            if new_dir == "-" {
                                target = previous_directory.to_str().unwrap();
                            } else {
                                target = new_dir;
                            }

                            // Perform variable expansion
                            let target = perform_expansion_on_single_element(target);
                            // Save the location we're in before changing directory
                            let dir_before_cd = env::current_dir().unwrap();

                            if let Err(e) = env::set_current_dir(Path::new(&target)) {
                                werror!("Error: {}: '{}'", e, target);
                                status = 1;
                                continue 'shell;
                            }

                            // Update the last directory if need be
                            if env::current_dir().unwrap() != dir_before_cd {
                                previous_directory =
                                    Path::new(dir_before_cd.to_str().unwrap()).to_path_buf();
                            }

                            previous_command = None;
                            status = 0
                        }
                        c => {
                            let stdin = previous_command
                                .map_or(Stdio::inherit(), |output: Child| {
                                    Stdio::from(output.stdout.unwrap())
                                });

                            let mut stdout = Stdio::inherit();
                            let mut stderr = Stdio::inherit();
                            if shell_command.piped {
                                stdout = Stdio::piped();
                                stderr = Stdio::piped();
                            } else if shell_command.redirection != Redirection::None {
                                stdout = Stdio::piped();
                                stderr = Stdio::piped();
                            }

                            let output = Command::new(c)
                                .args(shell_command.args)
                                .stdin(stdin)
                                .stdout(stdout)
                                .stderr(stderr)
                                .spawn();

                            match output {
                                Ok(output) => {
                                    status = 0;

                                    // Process redirection if need be
                                    if shell_command.redirection != Redirection::None {
                                        let _o = &output
                                            .wait_with_output()
                                            .expect("failed to wait on child");

                                        wwarning!("TODO: manage redirection");
                                        // redirect(&line, o);
                                        previous_command = None;
                                    } else {
                                        previous_command = Some(output);
                                    }
                                }
                                Err(e) => {
                                    previous_command = None;
                                    werror!("{}: {:?}", e, command);
                                    status = 1;
                                }
                            };
                        }
                    }
                }
                if let Some(mut final_command) = previous_command {
                    // block until the final command has finished
                    final_command.wait().unwrap();
                }
            }
            Err(ReadlineError::Interrupted) => (),
            Err(ReadlineError::Eof) => {
                return Ok(());
            }
            Err(err) => {
                werror!("Interactive error: {:?}. Exiting", err);
                break;
            }
        }
    }
    rl.save_history(&history).unwrap();

    Ok(())
}
