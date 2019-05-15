# Compactor

## A user interface to Windows 10 filesystem compression.

Windows 10 includes a new transparent file compression system, aimed at reducing
the disk space used by applications without increasing load times.

Many large applications, including games, compress surprisingly well, with savings
of 30% or more not being uncommon.

Sadly this is only exposed to users via a command line program &mdash; [`compact.exe`].

Compactor offers a pleasant, efficient, and responsive GUI alternative suitable for
keeping even very large directories compressed.


## Features

### Pause, Resume, Stop

Compactor is written in multithreaded [Rust], and its background operations can be
interrupted or paused at any time.  The only deficiency is during compression and
decompression - it must wait for the current file to finish processing.

### Compresstimation

Compactor performs a statistical compressibility check on larger files before
passing them off to Windows for compaction.  A large incompressible file can be
skipped in a fraction of a second instead of tying up the compact function for
minutes for no benefit.

### Machine Learning

Well, not really, but I had you going for a second there.  Compactor maintains a
database of paths that don't compress effectively, so they can be excluded from
future runs.

This database is, of course, compressed.  It's also safe to share between
multiple `Compactor` instances, in case you wanted to compact two drives at the
same time.

### Scalable and Fast

All this adds up to an application that will barely blink if you point it at a
few million files, and will recompress a previously compressed folder with new
or modified uncompressed files with minimal fuss.

Here it is having scanned my D:\Games folder:

<img src="https://i.imgur.com/VxyJmgR.png" style="width: 50%;height: auto;" alt="">


## Status

Compactor is currently considered alpha quality &mdash; not all features are
complete, some existing features could be implemented better, and it has received
only limited testing.

I *believe* it to be safe to use, and have used it to compress millions of files
without any problems, but I'm also keenly aware of how much software really only
works by accident, and advise you to keep backups of anything irreplaceable.

Just in general, really.  This is very far from the only thing that could break
something.


## Future

These may or may not happen, but have been on my mind.

* Write some bindings to Microsoft's [Compression API], add benchmarks for the
various compression modes to help users decide which is most appropriate for
their system.

* Examine overlapped IO mode, see if we can get more information and control out
of the compression process.

* Scheduled task to periodically recompress selected directories.

* Less rubbish installer.

* Sign the binaries/installer.


## Alternatives

[`compact.exe`] is a command-line tool that ships with Windows 10.  If you're
familiar with the command line and batch files, maybe you'd prefer that. Weirdo.

[CompactGUI] is a popular tool that uses `compact.exe` instead of native Windows
API calls.  It's considerably slower and more memory hungry, but does have the
advantage of maturity.  CompactGUI's seen over 100,000 downloads, Compactor has
as I write this not even passed single digits.

Are you aware of any others?  Do let me know.


## Nerdy Technical Stuff

Compactor is primarily written in [Rust].  The front-end is basically an embedded
website driven by the [web-view] crate (don't worry, it doesn't open any ports
or request any external resources).

Under the hood it uses [`DeviceIoControl`] with [`FSCTL_SET_EXTERNAL_BACKING`]
and [`FSCTL_DELETE_EXTERNAL_BACKING`], and a few functions from [WofApi].  This
is, of course, in part thanks to the [winapi] crate.  Eventually I hope to get
around to finishing off some of my bindings and contributing them back.

Compresstimation uses a simple linear sampling algorithm, passing blocks through
LZ4 level 1 as a compressibility check.

The incompressible-files database is a simple append-only list of paths stored in
LZ4-compressed packets.


## Author

Thomas Hurst - https://hur.st/ - tom@hur.st

I'm a nerdy, aloof weirdo from the north-east of England who's been programming
for about 25 years.


[`compact.exe`]: https://docs.microsoft.com/en-us/windows-server/administration/windows-commands/compact
[Rust]: https://www.rust-lang.org/
[CompactGUI]: https://github.com/ImminentFate/CompactGUI
[web-view]: https://github.com/Boscop/web-view
[`DeviceIoControl`]: https://docs.microsoft.com/en-us/windows/desktop/api/ioapiset/nf-ioapiset-deviceiocontrol
[`FSCTL_SET_EXTERNAL_BACKING`]: https://docs.microsoft.com/en-us/windows-hardware/drivers/ifs/fsctl-set-external-backing
[`FSCTL_DELETE_EXTERNAL_BACKING`]: https://docs.microsoft.com/en-us/windows-hardware/drivers/ifs/fsctl-delete-external-backing
[WofApi]: https://docs.microsoft.com/en-us/windows/desktop/api/wofapi/
[Compression API]: https://docs.microsoft.com/en-gb/windows/desktop/cmpapi/using-the-compression-api
[winapi](https://github.com/retep998/winapi-rs)
