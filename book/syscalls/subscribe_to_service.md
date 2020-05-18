# `subscribe_to_service`
Subscribe to a registered service by name. This will deliver a notification to the task that registered the
service with one end of a newly created channel. The other end of the channel will be returned by this system call,
if successful.

See the section on [Services](idk) for more information about services, how to register a service, and how to
subscribe to a service.

### Parameters
`a` - the length of the name string in bytes. Maximum length is 256. Must be greater than `0`.
`b` - a usermode pointer to the start of the UTF-8 encoded name string.

### Returns
Returns the standard representation of a `Result<Handle, ServiceError>`. Error status codes are:
- `1` if the task does not have the correct capability
- `2` if the usermode pointer to the name is not valid
- `3` if the name is too long, or `0`
- `4` if the supplied name does not correspond to a registered channel.

The returned handle is to one end of a `Channel`, the other end of which has been given to the task that supplies
the service.

### Capabilities needed
The `ServiceUser` capability is needed to make this system call.
