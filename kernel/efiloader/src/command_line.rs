use log::warn;

pub struct CommandLine<'a> {
    pub kernel_path: Option<&'a str>,
    pub graphics_mode: Option<GraphicsMode>,
}

pub struct GraphicsMode {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl<'a> CommandLine<'a> {
    pub fn new(string: &'a str) -> CommandLine<'a> {
        let mut command_line = CommandLine { kernel_path: None, graphics_mode: None };

        /*
         * The command line consists of a number of options, delimited by spaces. The first 'option' is the path
         * to the loader EFI executable, and so we skip it.
         */
        let mut options = string.split(' ');
        options.next();

        for option in options {
            /*
             * Each option is of the form `name(=value)`, where `name` is of the form `root(.extra)`.
             */
            let (name, value) = {
                match option.find('=') {
                    Some(index) => {
                        let (name, value) = option.split_at(index);
                        // Skip the '=' on the front of the value
                        (name, Some(&value[1..]))
                    }
                    None => (option, None),
                }
            };

            let (root, extra) = {
                match name.find('.') {
                    Some(index) => {
                        let (root, extra) = name.split_at(index);
                        // Skip the '.' at the front of `extra`
                        (root, Some(&extra[1..]))
                    }
                    None => (name, None),
                }
            };

            match root {
                "kernel" => {
                    command_line.kernel_path =
                        Some(value.expect("'kernel' parameter must have the path to the kernel image as a value"));
                }
                "graphics" => match extra.expect("'graphics' is not an option on its own") {
                    "width" => {
                        if command_line.graphics_mode.is_none() {
                            command_line.graphics_mode = Some(GraphicsMode {
                                width: Some(
                                    str::parse(value.expect("'graphics.width' has no value"))
                                        .expect("Value of 'graphics.width' must be an integer"),
                                ),
                                height: None,
                            });
                        } else {
                            command_line.graphics_mode.as_mut().unwrap().width = Some(
                                str::parse(value.expect("'graphics.width' has no value"))
                                    .expect("Value of 'graphics.width' must be an integer"),
                            );
                        }
                    }
                    "height" => {
                        if command_line.graphics_mode.is_none() {
                            command_line.graphics_mode = Some(GraphicsMode {
                                width: None,
                                height: Some(
                                    str::parse(value.expect("'graphics.height' has no value"))
                                        .expect("Value of 'graphics.height' must be an integer"),
                                ),
                            });
                        } else {
                            command_line.graphics_mode.as_mut().unwrap().height = Some(
                                str::parse(value.expect("'graphics.height' has no value"))
                                    .expect("Value of 'graphics.height' must be an integer"),
                            );
                        }
                    }
                    other => warn!("Unsupported graphics kernel command line option: '{}'. Ignoring.", other),
                },
                _ => warn!("Unsupported kernel command line option with root: '{}'. Ignoring.", root),
            }
        }

        command_line
    }
}
