# `register_service`
Register yourself as the provider of a service. The name of the service will be `{task_name}.{service_name}`. This 
returns a channel that is used to alert the provider when another task subscribes to your service with the
`subscribe_to_service` system call.

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

The returned handle is to a `Channel` that is used to serve channel subscriptions.

### Capabilities needed
The `ServiceProvider` capability is needed to make this system call.
