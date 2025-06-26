# Future Ideas
This is a collection of ideas that I like, but are quite far off in the development roadmap, and so sit here undeveloped so I don't forget
about them.

## Updating a Poplar system
I like the idea I've seen of having like an A and B partition so that a system that is half-updated and then e.g loses power doesn't
become unbootable. I think a simpler way of doing this with an EFI partition that also adds some security (I half wonder if this is what
e.g. Windows is doing anyway) is to utilise our future VFS design like so:
- `boot:` is a read-only mounted version of the EFI partition that contains e.g. the running kernel, programs, etc.
- `boot-upgrade:` is a read-write mounted version of a folder inside the EFI partition, where upgraded versions can be put

An upgrade would happen like so:
- E.g. `boot:/EFI/Boot/Bootx64.efi` and `boot:/kernel.elf` are current versions of Seed and the kernel respectively
- The userspace upgrade service would put new versions at `boot-update:/kernel.elf`, which is also `boot:/upgrade/kernel.elf`
- The system reboots
- Seed looks inside its `/upgrade` to see if it should do an upgrade, and if so moves these versions to the base EFI FS
- If Seed itself has been updated, the system reboots itself again
- The (potentially) new Seed loads and boots into the new kernel + base userspace tasks

We should also probably have a mechanism for verifying an update is complete/correct before deleting the old files from Seed.

## New AML interpreter
Our AML interpreter is pretty bad. The key insight is to realise that AML is a weird hybrid of a bad VM bytecode (and so needs to be
interpreted as such, i.e. non-recursively) and also really a compressed form of the original source ASL. This has led to Cool Facts such
as it having basically limitless nested expressions (which can easily explode your stack usage in-kernel), while needing to be interpreted
in a left-right order (as opposed to e.g. stack based).

We're going to use the same technique as uACPI and some other interpreters to tackle this. Your core interpreter needs to look like a real
VM (which means having a single loop going through, indexing by some `pc`). We'll then track a number of "in-flight" operations, which are
operations that have been started, but we don't have all the arguments for yet. This is what enforces the left-to-right interpreting order.

Each new opcode at the start of the loop creates a new *in-flight* operation. This could complete immediately (e.g. it encodes an integer),
or could require significant computation (e.g. a field read requiring method invocations and a PCI config space access). This is the key to
this interpreter design - the latter doesn't occur 7000 stack frames deep.

When an operation can be completed, we then need to track where its result should go. Specifically, if it was an argument to another in-flight
operation, this could mean that we can now complete the next operation up the chain. This could in turn complete another op. Correct behaviour
is to complete as many operations as possible, in an eager fasion.

(Not actually sure on second-look whether this psuedo-code is correct re arg handling, but it's roughly the right shape):
``` rust
struct InFlightOp {
    op: Op,
    num_args_needed: u8,
    args: Vec<Value>,
}

let inflight_ops: Vec<InFlightOp>;

loop {
    let next_op = self.get_op();    // Pop 1 or 2 bytes off to identify the next operation. Returns enum of opcodes
    let spec = self.specs[next_op]; // Look up the specification of the opcode
    
    if spec.encodes_arg {
        let value = ...;
        let op = inflight_ops.last_mut();
        op.args.push(value);

        if op.args.len() == op.num_args_needed {
            todo!("do a bunch of logic to complete/retire this operation now it's finished");
            todo!("recurse down the inflight ops seeing if that has made another one complete. do this greedily");
        }
    } else {
        inflight_ops.push(InFlightOp::new(next_op, spec.num_args_needed));
    }
}
```

Other thoughts:
- We need to think about how to represent values and how references are going to work
- I think we need to submit and allow locks to various things (e.g. the namespace). This will require very careful locking :(
- Other things might actually be a little simpler (e.g. the basic parsing) than with the combinators
- Nested method calls can be handled with a heap-allocated stack of call frames

## Common logging framework
Our current logging framework is very rudimentary, and is largely duplicated between architectures. The next iteration should
be live in the common kernel, and think ahead to supporting multi-core logging and strategies other than shoving everything out
via a serial port, which real systems will not want to do.

I am still undecided over whether real structured logging (a la `tracing` instead of `log`) makes sense in Poplar. I don't really
use structured logging, but this may just be because I am not used to it yet. We should look in more detail over whether we really
care about supporting it, as it does add much complexity.

Linux uses a pretty-large set of ring buffers, one of descriptors and another of actual formatted text. It has very complex lockless
synchronization to make it NMI-safe etc. that we may or may not want to involve ourselves with. This ring buffer is exposed to userspace
via a file, and is read by `dmesg`.