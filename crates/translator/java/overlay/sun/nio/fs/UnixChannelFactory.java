package sun.nio.fs;

import java.io.InputStream;
import java.io.OutputStream;
import java.nio.channels.AsynchronousFileChannel;
import java.nio.channels.FileChannel;
import java.nio.file.LinkOption;
import java.nio.file.OpenOption;
import java.nio.file.StandardOpenOption;
import java.util.Set;
import sun.nio.ch.ThreadPool;

import static sun.nio.fs.UnixConstants.*;
import static sun.nio.fs.UnixNativeDispatcher.*;

/**
 * The translator's {@code UnixChannelFactory}, which replaces the JDK's in a translation with
 * the JDK's sources. The JDK's wraps the descriptor it opens in a {@code FileDescriptor}, through
 * {@code SharedSecrets}, for {@code sun.nio.ch.FileChannelImpl}, which reads and writes through
 * memory the JVM gives. Here the descriptor, one of the runtime's table, goes to the translator's
 * {@code UnixFileChannel}, or to its streams for {@code Files.newInputStream} and
 * {@code newOutputStream} ({@code LinuxFileSystemProvider}). The options and the opening are the
 * JDK's. There are no asynchronous file channels: {@code newAsynchronousFileChannel} throws
 * {@code UnsupportedOperationException}.
 */
class UnixChannelFactory {
    protected UnixChannelFactory() {
    }

    /**
     * Represents the flags from a user-supplied set of open options.
     */
    protected static class Flags {
        boolean read;
        boolean write;
        boolean append;
        boolean truncateExisting;
        boolean noFollowLinks;
        boolean create;
        boolean createNew;
        boolean deleteOnClose;
        boolean sync;
        boolean dsync;
        boolean direct;

        static Flags toFlags(Set<? extends OpenOption> options) {
            Flags flags = new Flags();
            for (OpenOption option: options) {
                if (option instanceof StandardOpenOption) {
                    switch ((StandardOpenOption)option) {
                        case READ : flags.read = true; break;
                        case WRITE : flags.write = true; break;
                        case APPEND : flags.append = true; break;
                        case TRUNCATE_EXISTING : flags.truncateExisting = true; break;
                        case CREATE : flags.create = true; break;
                        case CREATE_NEW : flags.createNew = true; break;
                        case DELETE_ON_CLOSE : flags.deleteOnClose = true; break;
                        case SPARSE : /* ignore */ break;
                        case SYNC : flags.sync = true; break;
                        case DSYNC : flags.dsync = true; break;
                        default: throw new UnsupportedOperationException();
                    }
                    continue;
                }
                if (option == LinkOption.NOFOLLOW_LINKS && O_NOFOLLOW != 0) {
                    flags.noFollowLinks = true;
                    continue;
                }

                if (ExtendedOptions.DIRECT.matches(option)) {
                    flags.direct = true;
                    continue;
                }

                if (option == null)
                    throw new NullPointerException();
               throw new UnsupportedOperationException(option + " not supported");
            }
            return flags;
        }
    }

    /** The flags of a channel: reading by default, writing for {@code APPEND}; validated. */
    private static Flags channelFlags(Set<? extends OpenOption> options) {
        Flags flags = Flags.toFlags(options);

        // default is reading; append => writing
        if (!flags.read && !flags.write) {
            if (flags.append) {
                flags.write = true;
            } else {
                flags.read = true;
            }
        }

        // validation
        if (flags.read && flags.append)
            throw new IllegalArgumentException("READ + APPEND not allowed");
        if (flags.append && flags.truncateExisting)
            throw new IllegalArgumentException("APPEND + TRUNCATE_EXISTING not allowed");
        return flags;
    }

    /**
     * Constructs a file channel by opening a file using a dfd/path pair
     */
    static FileChannel newFileChannel(int dfd,
                                      UnixPath path,
                                      Set<? extends OpenOption> options,
                                      int mode)
        throws UnixException
    {
        Flags flags = channelFlags(options);
        int fd = open(dfd, path, flags, mode);
        return new UnixFileChannel(fd, flags.read, flags.write, flags.append);
    }

    /**
     * Constructs a file channel by opening the given file.
     */
    static FileChannel newFileChannel(UnixPath path,
                                      Set<? extends OpenOption> options,
                                      int mode)
        throws UnixException
    {
        return newFileChannel(-1, path, options, mode);
    }

    /** The stream of a file opened as {@link #newFileChannel} opens it, for reading. */
    static InputStream newInputStream(UnixPath path, Set<? extends OpenOption> options, int mode)
        throws UnixException
    {
        return new UnixFileInputStream(open(-1, path, channelFlags(options), mode));
    }

    /** The stream of a file opened as {@link #newFileChannel} opens it, for writing. */
    static OutputStream newOutputStream(UnixPath path, Set<? extends OpenOption> options, int mode)
        throws UnixException
    {
        return new UnixFileOutputStream(open(-1, path, channelFlags(options), mode));
    }

    /** There are no asynchronous file channels in a translated program. */
    static AsynchronousFileChannel newAsynchronousFileChannel(UnixPath path,
                                                              Set<? extends OpenOption> options,
                                                              int mode,
                                                              ThreadPool pool)
        throws UnixException
    {
        throw new UnsupportedOperationException("asynchronous file channels in a translated program");
    }

    /**
     * Opens file based on parameters and options, returning the runtime's descriptor of it.
     */
    protected static int open(int dfd,
                              UnixPath path,
                              Flags flags,
                              int mode)
        throws UnixException
    {
        // map to oflags
        int oflags;
        if (flags.read && flags.write) {
            oflags = O_RDWR;
        } else {
            oflags = (flags.write) ? O_WRONLY : O_RDONLY;
        }
        if (flags.write) {
            if (flags.truncateExisting)
                oflags |= O_TRUNC;
            if (flags.append)
                oflags |= O_APPEND;

            // create flags
            if (flags.createNew) {
                byte[] pathForSysCall = path.getByteArrayForSysCalls();

                // throw exception if file name is "." to avoid confusing error
                if ((pathForSysCall[pathForSysCall.length-1] == '.') &&
                    (pathForSysCall.length == 1 ||
                    (pathForSysCall[pathForSysCall.length-2] == '/')))
                {
                    throw new UnixException(EEXIST);
                }
                oflags |= (O_CREAT | O_EXCL);
            } else {
                if (flags.create)
                    oflags |= O_CREAT;
            }
        }

        // follow links by default
        boolean followLinks = true;
        if (!flags.createNew && (flags.noFollowLinks || flags.deleteOnClose)) {
            if (flags.deleteOnClose && O_NOFOLLOW == 0) {
                try {
                    if (UnixFileAttributes.get(path, false).isSymbolicLink())
                        throw new UnixException("DELETE_ON_CLOSE specified and file is a symbolic link");
                } catch (UnixException x) {
                    if (!flags.create || x.errno() != ENOENT)
                        throw x;
                }
            }
            followLinks = false;
            oflags |= O_NOFOLLOW;
        }

        if (flags.dsync)
            oflags |= O_DSYNC;
        if (flags.sync)
            oflags |= O_SYNC;
        if (flags.direct)
            oflags |= O_DIRECT;

        int fd;
        try {
            if (dfd >= 0) {
                fd = openat(dfd, path.asByteArray(), oflags, mode);
            } else {
                fd = UnixNativeDispatcher.open(path, oflags, mode);
            }
        } catch (UnixException x) {
            // Linux error can be EISDIR or EEXIST when file exists
            if (flags.createNew && (x.errno() == EISDIR)) {
                x.setError(EEXIST);
            }

            // handle ELOOP to avoid confusing message
            if (!followLinks && (x.errno() == ELOOP)) {
                x = new UnixException(x.getMessage() + " (NOFOLLOW_LINKS specified)");
            }

            throw x;
        }

        // unlink file immediately if delete on close. The spec is clear that
        // an implementation cannot guarantee to unlink the correct file when
        // replaced by an attacker after it is opened.
        if (flags.deleteOnClose) {
            try {
                if (dfd >= 0) {
                    unlinkat(dfd, path.asByteArray(), 0);
                } else {
                    unlink(path);
                }
            } catch (UnixException ignore) {
                // best-effort
            }
        }
        return fd;
    }
}
