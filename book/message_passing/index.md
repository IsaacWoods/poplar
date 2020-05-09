# Message Passing
Pebble has a kernel object called a `Channel` for providing first-class message passing support to userspace.
Channels move packets, called "messages", which contain a stream of bytes, and optionally one or more handles that
are transferred from the sending task to the receiving task.

While Channels can move arbitrary bytes, most of Pebble will use a common set of tools to exchange data over
channels, including a wire format that is suitable for many types of structured data.
