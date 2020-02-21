use crate::LoaderError;
use log::warn;

pub struct CommandLine<'a> {
    pub volume_label: Result<&'a str, LoaderError>,
    pub kernel_path: Result<&'a str, LoaderError>,
    pub graphics_mode: Option<GraphicsMode>,
}

pub struct GraphicsMode {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl<'a> CommandLine<'a> {
    pub fn new(string: &'a str) -> CommandLine<'a> {
        let mut command_line = CommandLine {
            volume_label: Err(LoaderError::NoBootVolume),
            kernel_path: Err(LoaderError::NoKernelPath),
            graphics_mode: None,
        };

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
                "volume" => {
                    command_line.volume_label =
                        Ok(value.expect("'volume' parameter must have a volume label as a value"));
                }
                "kernel" => {
                    command_line.kernel_path =
                        Ok(value.expect("'kernel' parameter must have the path to the kernel image as a value"));
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
