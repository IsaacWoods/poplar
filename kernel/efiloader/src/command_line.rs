use crate::LoaderError;
use log::warn;
use x86_64::memory::KIBIBYTES_TO_BYTES;

const DEFAULT_VOLUME_LABEL: &'static str = "BOOT";
const DEFAULT_KERNEL_HEAP_SIZE: usize = 200 * KIBIBYTES_TO_BYTES;
const MAX_IMAGES: usize = 32;

pub struct CommandLine<'a> {
    pub volume_label: &'a str,
    pub kernel_path: Result<&'a str, LoaderError>,
    pub framebuffer: Option<Framebuffer>,
    /// The size of the kernel heap that should be allocated, in bytes.
    pub kernel_heap_size: usize,
    pub num_images: usize,
    /// A list of the images we've been asked to load, in the form `(name, path)`
    pub images: [Option<(&'a str, &'a str)>; MAX_IMAGES],
}

#[derive(Clone, Copy)]
pub struct Framebuffer {
    pub width: Option<usize>,
    pub height: Option<usize>,
}

impl<'a> CommandLine<'a> {
    pub fn new(string: &'a str) -> CommandLine<'a> {
        let mut command_line = CommandLine {
            volume_label: DEFAULT_VOLUME_LABEL,
            kernel_path: Err(LoaderError::NoKernelPath),
            framebuffer: None,
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
                        value.expect("'volume' parameter must have a volume label as a value");
                }
                "kernel" => {
                    command_line.kernel_path =
                        Ok(value.expect("'kernel' parameter must have the path to the kernel image as a value"));
                }
                "fb" => match extra.expect("'fb' is not an option on its own") {
                    "width" => {
                        if command_line.framebuffer.is_none() {
                            command_line.framebuffer = Some(Framebuffer {
                                width: Some(
                                    str::parse(value.expect("'fb.width' has no value"))
                                        .expect("Value of 'fb.width' must be an integer"),
                                ),
                                height: None,
                            });
                        } else {
                            command_line.framebuffer.as_mut().unwrap().width = Some(
                                str::parse(value.expect("'fb.width' has no value"))
                                    .expect("Value of 'fb.width' must be an integer"),
                            );
                        }
                    }
                    "height" => {
                        if command_line.framebuffer.is_none() {
                            command_line.framebuffer = Some(Framebuffer {
                                width: None,
                                height: Some(
                                    str::parse(value.expect("'fb.height' has no value"))
                                        .expect("Value of 'fb.height' must be an integer"),
                                ),
                            });
                        } else {
                            command_line.framebuffer.as_mut().unwrap().height = Some(
                                str::parse(value.expect("'fb.height' has no value"))
                                    .expect("Value of 'fb.height' must be an integer"),
                            );
                        }
                    }
                    other => warn!("Unsupported framebuffer setting: '{}'. Ignoring.", other),
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
