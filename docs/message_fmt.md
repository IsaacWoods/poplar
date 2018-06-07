# Encoding and Decoding messages in Pebble
To send and receive messages between nodes, we need to encode and decode them reliably. This process is incredibly important - message passing is the largest attack surface accessible from programs in
userland, so the kernel must be able to correctly parse all malformed messages without violating safety.

```
Message := MessageHeader Payload

MessageHeader := SenderId ReceiverId PayloadLength
SenderId := NodeId
ReceiverId := NodeId
NodeId := {u16}
PayloadLength := {u8}

Payload := MessageType ...
MessageType := {u8} => PrintDebugMessage(0x00)


```

### Possible Attacks
* Incorrect length - what if the payload is shorter than the specified number of bytes? **As soon as we detect a malformed message, we should clear the entire Send Buffer and send a message to the Sender to
alert them of the malformed message**
* What if the Sender tries to spoof the `SenderId`? **We should validate that the `SenderId` of every message matches the `NodeId` of the Send Buffer's process**
* What if the `ReceiverId` doesn't point to a valid node? **We should discard the message and alert the sender**

### Implementation notes
* `MessageType` **must** be checked to be valid before being converted back to a real Rust type in the kernel. Constructing an invalid enum variant is undefined behaviour, and must not be done!
