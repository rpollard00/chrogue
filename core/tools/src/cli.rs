//! The command line of the binaries: options of the form `--name VALUE`, and the other arguments.

/// The value of an option. Panics if the option has no value.
pub fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .map(|i| args.get(i + 1).unwrap_or_else(|| panic!("{name} needs a value")).as_str())
}

/// The number of an option, or the default. Panics if the value is not a number.
pub fn number(args: &[String], name: &str, default: u64) -> u64 {
    option(args, name).map_or(default, |text| text.parse().unwrap_or_else(|_| panic!("{name} must be a number")))
}

/// The number of threads: `--threads`, or one for each processor.
pub fn threads(args: &[String]) -> usize {
    let default = std::thread::available_parallelism().map_or(1, |n| n.get()) as u64;
    number(args, "--threads", default).max(1) as usize
}

/// The arguments that are not an option and not the value of an option.
pub fn positionals(args: &[String]) -> Vec<&str> {
    let mut list = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg.starts_with("--") {
            index += 2;
        } else {
            list.push(arg.as_str());
            index += 1;
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_positionals_are_not_the_options_and_not_their_values() {
        let args: Vec<String> = ["--white", "level4", "level1", "--games", "5", "level2"].map(String::from).to_vec();
        assert_eq!(positionals(&args), ["level1", "level2"]);
        assert_eq!((option(&args, "--white"), option(&args, "--seed")), (Some("level4"), None));
        assert_eq!((number(&args, "--games", 100), number(&args, "--seed", 1)), (5, 1));
    }
}
