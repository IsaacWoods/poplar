use crate::LoaderError;
use log::warn;
use x86_64::memory::KIBIBYTES_TO_BYTES;

const DEFAULT_KERNEL_HEAP_SIZE: usize = 200 * KIBIBYTES_TO_BYTES;
const MAX_IMAGES: usize = 32;

pub struct CommandLine<'a> {
    pub volume_label: Result<&'a str, LoaderError>,
    pub kernel_path: Result<&'a str, LoaderError>,
    pub graphics_mode: Option<GraphicsMode>,
    /// The size of the kernel heap that should be allocated, in bytes.
    pub kernel_heap_size: usize,
    pub num_images: usize,
    /// A list of the images we've been asked to load, in the form `(name, path)`
    pub images: [Option<(&'a str, &'a str)>; MAX_IMAGES],
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
            kernel_heap_size: DEFAULT_KERNEL_HEAP_SIZE,
            num_images: 0,
            images: [None; MAX_IMAGES],
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
                "image" => {
                    let name = extra.expect("An image must have a name, supplied as 'image.your_name_here'");
                    let path = value.expect("You've specified an image without a path!");
                    command_line.add_image(name, path);
                }
                _ => warn!("Unsupported kernel command line option with root: '{}'. Ignoring.", root),
            }
        }

        command_line
    }

    pub fn images(&self) -> &[Option<(&'a str, &'a str)>] {
        &self.images[0..self.num_images]
    }

    fn add_image(&mut self, name: &'a str, path: &'a str) {
        if self.num_images >= MAX_IMAGES {
            panic!("Too many images supplied to loader!");
        }

        self.images[self.num_images] = Some((name, path));
        self.num_images += 1;
    }
}
