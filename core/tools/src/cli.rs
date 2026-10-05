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

/// The numbers of a list of numbers and ranges with `,` between them, such as `1-3,8`. Each
/// number must be from `min` to `max`. The result has each number one time, in the order of the text.
pub fn ranges(text: &str, min: usize, max: usize) -> Result<Vec<usize>, String> {
    let mut list = Vec::new();
    for part in text.split(',') {
        let number = |text: &str| {
            text.parse::<usize>()
                .ok()
                .filter(|n| (min..=max).contains(n))
                .ok_or_else(|| format!("\"{text}\" must be a number from {min} to {max}"))
        };
        let (first, last) = match part.split_once('-') {
            Some((first, last)) => (number(first)?, number(last)?),
            None => (number(part)?, number(part)?),
        };
        if first > last {
            return Err(format!("\"{part}\" must go from the smaller number to the larger number"));
        }
        for n in first..=last {
            if !list.contains(&n) {
                list.push(n);
            }
        }
    }
    Ok(list)
}

/// The first argument that is not one of these options and not the value of one.
pub fn unknown<'a>(args: &'a [String], options: &[&str]) -> Option<&'a str> {
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if !options.contains(&arg.as_str()) {
            return Some(arg);
        }
        index += 2;
    }
    None
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

    #[test]
    fn unknown_gives_the_first_argument_that_is_not_an_option() {
        let args: Vec<String> = ["--games", "5", "--floors", "3"].map(String::from).to_vec();
        assert_eq!(unknown(&args, &["--games", "--floor"]), Some("--floors"));
        assert_eq!(unknown(&args, &["--games", "--floors"]), None);
        assert_eq!(unknown(&args[..3], &["--games"]), Some("--floors"));
    }

    #[test]
    fn ranges_gives_each_number_one_time() {
        assert_eq!(ranges("1-3,8,2", 1, 8), Ok(vec![1, 2, 3, 8]));
        assert_eq!(ranges("4", 1, 8), Ok(vec![4]));
        for bad in ["0", "9", "3-1", "a", "1-", ""] {
            assert!(ranges(bad, 1, 8).is_err(), "{bad}");
        }
    }
}
