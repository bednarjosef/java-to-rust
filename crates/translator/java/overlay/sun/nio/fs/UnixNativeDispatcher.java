package sun.nio.fs;

import java.util.function.Function;

/**
 * The translator's {@code UnixNativeDispatcher}, the system calls of {@code java.nio.file},
 * which replaces the JDK's in a translation with the JDK's sources.
 *
 * <p>The JDK's class copies each path into memory of its own ({@code NativeBuffer}, through
 * {@code Unsafe}) and gives the system call its address. This one gives the natives the bytes of
 * the path, and the runtime makes the calls ({@code crates/runtime/src/unix.rs}). Its natives do
 * not throw: each one records the {@code errno} of its failure, which {@link #lastError} reads,
 * and the methods here throw the {@code UnixException} that the JDK's natives would.
 *
 * <p>Descriptors are those of the runtime's table, which {@code FileInputStream} and the others
 * use too. The calls relative to an open directory ({@code openat}, {@code fdopendir} and the
 * others), which the JDK makes only where {@link #openatSupported} says so, fail with
 * {@code ENOSYS}. The calls on memory addresses (extended attributes, the mount table) and those
 * that look users and groups up by name or ask about a whole file system ({@code statvfs}) are
 * natives that the translator does not implement, so that a program that runs them is rejected.
 */
class UnixNativeDispatcher {
    protected UnixNativeDispatcher() { }

    /** The number of fields {@code stat0} and {@code fstat0} fill (see {@link UnixFileAttributes}). */
    static final int STAT_FIELDS = 17;

    /** The {@code errno} of this thread's last call of a native here, 0 where it succeeded. */
    private static native int lastError();

    /**
     * The failure of a call relative to an open directory, which the JDK makes only where
     * {@link #openatSupported} says it may, or where a directory stream it got that way gives the
     * descriptor; neither happens here.
     */
    private static UnixException notSupported() {
        return new UnixException(UnixConstants.ENOSYS);
    }

    /** Throws the error of the last call, if it failed. */
    private static void check() throws UnixException {
        int errno = lastError();
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    static byte[] getcwd() {
        return getcwd0();
    }
    private static native byte[] getcwd0();

    static int dup(int filedes) throws UnixException {
        int fd = dup0(filedes);
        check();
        return fd;
    }
    private static native int dup0(int filedes);

    static int open(UnixPath path, int flags, int mode) throws UnixException {
        int fd = open0(path.getByteArrayForSysCalls(), flags, mode);
        check();
        return fd;
    }
    private static native int open0(byte[] path, int flags, int mode);

    static int openat(int dfd, byte[] path, int flags, int mode) throws UnixException {
        throw notSupported();
    }

    static void close(int fd) throws UnixException {
        if (fd != -1) {
            close0(fd);
            check();
        }
    }
    private static native void close0(int fd);

    static <X extends Throwable> void close(int fd, Function<UnixException, X> mapper) throws X {
        try {
            close(fd);
        } catch (UnixException e) {
            if (mapper != null) {
                X ex = mapper.apply(e);
                if (ex != null) throw ex;
            }
        }
    }

    static native void rewind(long stream) throws UnixException;

    static native int getlinelen(long stream) throws UnixException;

    static void link(UnixPath existing, UnixPath newfile) throws UnixException {
        link0(existing.getByteArrayForSysCalls(), newfile.getByteArrayForSysCalls());
        check();
    }
    private static native void link0(byte[] existing, byte[] newfile);

    static void unlink(UnixPath path) throws UnixException {
        unlink0(path.getByteArrayForSysCalls());
        check();
    }
    private static native void unlink0(byte[] path);

    static void unlinkat(int dfd, byte[] path, int flag) throws UnixException {
        throw notSupported();
    }

    static void mknod(UnixPath path, int mode, long dev) throws UnixException {
        mknod0(path.getByteArrayForSysCalls(), mode, dev);
    }
    private static native void mknod0(byte[] path, int mode, long dev) throws UnixException;

    static void rename(UnixPath from, UnixPath to) throws UnixException {
        rename0(from.getByteArrayForSysCalls(), to.getByteArrayForSysCalls());
        check();
    }
    private static native void rename0(byte[] from, byte[] to);

    static void renameat(int fromfd, byte[] from, int tofd, byte[] to) throws UnixException {
        throw notSupported();
    }

    static void mkdir(UnixPath path, int mode) throws UnixException {
        mkdir0(path.getByteArrayForSysCalls(), mode);
        check();
    }
    private static native void mkdir0(byte[] path, int mode);

    static void rmdir(UnixPath path) throws UnixException {
        rmdir0(path.getByteArrayForSysCalls());
        check();
    }
    private static native void rmdir0(byte[] path);

    static byte[] readlink(UnixPath path) throws UnixException {
        byte[] target = readlink0(path.getByteArrayForSysCalls());
        check();
        return target;
    }
    private static native byte[] readlink0(byte[] path);

    static byte[] realpath(UnixPath path) throws UnixException {
        byte[] real = realpath0(path.getByteArrayForSysCalls());
        check();
        return real;
    }
    private static native byte[] realpath0(byte[] path);

    static void symlink(byte[] name1, UnixPath name2) throws UnixException {
        symlink0(name1, name2.getByteArrayForSysCalls());
        check();
    }
    private static native void symlink0(byte[] name1, byte[] name2);

    /** {@code stat}, or {@code lstat} where {@code flag} is {@code AT_SYMLINK_NOFOLLOW}: the errno. */
    private static int stat(byte[] path, int flag, UnixFileAttributes attrs) {
        long[] fields = new long[STAT_FIELDS];
        stat0(path, flag, fields);
        int errno = lastError();
        if (errno == 0) {
            attrs.fill(fields);
        }
        return errno;
    }
    private static native void stat0(byte[] path, int flag, long[] fields);

    static void stat(UnixPath path, UnixFileAttributes attrs) throws UnixException {
        int errno = stat2(path, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    static int stat2(UnixPath path, UnixFileAttributes attrs) {
        return stat(path.getByteArrayForSysCalls(), 0, attrs);
    }

    static void lstat(UnixPath path, UnixFileAttributes attrs) throws UnixException {
        int errno = stat(path.getByteArrayForSysCalls(), UnixConstants.AT_SYMLINK_NOFOLLOW, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    static void fstat(int fd, UnixFileAttributes attrs) throws UnixException {
        long[] fields = new long[STAT_FIELDS];
        fstat0(fd, fields);
        check();
        attrs.fill(fields);
    }
    private static native void fstat0(int fd, long[] fields);

    static void fstatat(int dfd, UnixPath path, int flag, UnixFileAttributes attrs)
        throws UnixException
    {
        int errno = fstatat2(dfd, path, flag, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    /** {@code fstatat} relative to the working directory, which is {@code stat} or {@code lstat}. */
    static int fstatat2(int dfd, UnixPath path, int flag, UnixFileAttributes attrs) {
        if (dfd != UnixConstants.AT_FDCWD) {
            return UnixConstants.ENOSYS;
        }
        return stat(path.getByteArrayForSysCalls(), flag, attrs);
    }

    static void chown(UnixPath path, int uid, int gid) throws UnixException {
        chown0(path.getByteArrayForSysCalls(), uid, gid, 0);
        check();
    }
    private static native void chown0(byte[] path, int uid, int gid, int flag);

    static void lchown(UnixPath path, int uid, int gid) throws UnixException {
        chown0(path.getByteArrayForSysCalls(), uid, gid, UnixConstants.AT_SYMLINK_NOFOLLOW);
        check();
    }

    static void fchown(int fd, int uid, int gid) throws UnixException {
        fchown0(fd, uid, gid);
        check();
    }
    private static native void fchown0(int fd, int uid, int gid);

    static void chmod(UnixPath path, int mode) throws UnixException {
        chmod0(path.getByteArrayForSysCalls(), mode);
        check();
    }
    private static native void chmod0(byte[] path, int mode);

    static void fchmod(int fd, int mode) throws UnixException {
        fchmod0(fd, mode);
        check();
    }
    private static native void fchmod0(int fd, int mode);

    static void fchmodat(int fd, UnixPath path, int mode, int flag) throws UnixException {
        throw notSupported();
    }

    static void futimens(int fd, long times0, long times1) throws UnixException {
        futimens0(fd, times0, times1);
        check();
    }
    private static native void futimens0(int fd, long times0, long times1);

    static void utimensat(int fd, UnixPath path, long times0, long times1, int flags)
        throws UnixException
    {
        if (fd != UnixConstants.AT_FDCWD) {
            throw notSupported();
        }
        utimensat0(path.getByteArrayForSysCalls(), times0, times1, flags);
        check();
    }
    private static native void utimensat0(byte[] path, long times0, long times1, int flags);

    static long opendir(UnixPath path) throws UnixException {
        long dir = opendir0(path.getByteArrayForSysCalls());
        check();
        return dir;
    }
    private static native long opendir0(byte[] path);

    static long fdopendir(int dfd) throws UnixException {
        throw notSupported();
    }

    static void closedir(long dir) throws UnixException {
        closedir0(dir);
        check();
    }
    private static native void closedir0(long dir);

    /** The name of the next entry of the directory, or null after the last. */
    static byte[] readdir(long dir) throws UnixException {
        byte[] name = readdir0(dir);
        check();
        return name.length == 0 ? null : name;
    }
    private static native byte[] readdir0(long dir);

    static int read(int fildes, long buf, int nbyte) throws UnixException {
        return read0(fildes, buf, nbyte);
    }
    private static native int read0(int fildes, long buf, int nbyte) throws UnixException;

    static int write(int fildes, long buf, int nbyte) throws UnixException {
        return write0(fildes, buf, nbyte);
    }
    private static native int write0(int fildes, long buf, int nbyte) throws UnixException;

    /** {@code access(path, amode)}: 0, or the errno of why not. */
    static int access(UnixPath path, int amode) {
        return access0(path.getByteArrayForSysCalls(), amode);
    }
    private static native int access0(byte[] path, int amode);

    /** The name of the user {@code uid}, or a {@code UnixException} with errno 0 where there is none. */
    static byte[] getpwuid(int uid) throws UnixException {
        byte[] name = getpwuid0(uid);
        if (name.length == 0) {
            throw new UnixException(lastError());
        }
        return name;
    }
    private static native byte[] getpwuid0(int uid);

    /** The name of the group {@code gid}, or a {@code UnixException} with errno 0 where there is none. */
    static byte[] getgrgid(int gid) throws UnixException {
        byte[] name = getgrgid0(gid);
        if (name.length == 0) {
            throw new UnixException(lastError());
        }
        return name;
    }
    private static native byte[] getgrgid0(int gid);

    static int getpwnam(String name) throws UnixException {
        return getpwnam0(Util.toBytes(name));
    }
    private static native int getpwnam0(byte[] name) throws UnixException;

    static int getgrnam(String name) throws UnixException {
        return getgrnam0(Util.toBytes(name));
    }
    private static native int getgrnam0(byte[] name) throws UnixException;

    static void statvfs(UnixPath path, UnixFileStoreAttributes attrs) throws UnixException {
        statvfs0(path.getByteArrayForSysCalls(), attrs);
    }
    private static native void statvfs0(byte[] path, UnixFileStoreAttributes attrs) throws UnixException;

    static byte[] strerror(int errnum) {
        return strerror0(errnum);
    }
    private static native byte[] strerror0(int errnum);

    static int fgetxattr(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException
    {
        return fgetxattr0(filedes, name, valueAddress, valueLen);
    }
    private static native int fgetxattr0(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException;

    static void fsetxattr(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException
    {
        fsetxattr0(filedes, name, valueAddress, valueLen);
    }
    private static native void fsetxattr0(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException;

    static void fremovexattr(int filedes, byte[] name) throws UnixException {
        fremovexattr0(filedes, name);
    }
    private static native void fremovexattr0(int filedes, byte[] name) throws UnixException;

    static native int flistxattr(int filedes, long listAddress, int size) throws UnixException;

    /**
     * The natives of the translator's {@code FileChannel} ({@code UnixFileChannel}), on a
     * descriptor of the runtime's table: they read or write at the descriptor's position, or at
     * {@code position} where it is not negative, and answer -1 where they fail.
     */
    static native int readBytes(int fd, byte[] buf, int off, int len, long position);
    static native int writeBytes(int fd, byte[] buf, int off, int len, long position);
    static native long position(int fd);
    static native void seek(int fd, long position);
    static native long size(int fd);
    static native void truncate(int fd, long size);
    static native void sync(int fd, boolean metadata);

    /** Throws the error of the last call of the natives above, if it failed. */
    static void checkLast() throws UnixException {
        check();
    }

    // Neither `openat` nor extended attributes: `newDirectoryStream` gives the plain stream, and
    // the DOS and user-defined attribute views are refused. The runtime reads when a file was
    // created where the system knows it (`statx`), as the JDK on Linux does.
    private static final int SUPPORTS_BIRTHTIME = 1 << 16;
    private static final int capabilities = SUPPORTS_BIRTHTIME;

    static boolean openatSupported() {
        return false;
    }

    static boolean birthtimeSupported() {
        return (capabilities & SUPPORTS_BIRTHTIME) != 0;
    }

    static boolean xattrSupported() {
        return false;
    }

    static boolean fchmodatNoFollowSupported() {
        return false;
    }
}
