package sun.nio.fs;

/**
 * The translator's patch of the JDK's {@code UnixNativeDispatcher}, the system calls of
 * {@code java.nio.file} (see docs/jdk-overlays.md).
 *
 * <p>The JDK's class copies each path into memory of its own ({@code NativeBuffer}, through
 * {@code Unsafe}) and gives the system call its address. Patched, it gives the natives the bytes of
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
    /** The number of fields {@code stat0} and {@code fstat0} fill (see {@link UnixFileAttributes}). */
    @Add
    static final int STAT_FIELDS = 17;

    /** The {@code errno} of this thread's last call of a native here, 0 where it succeeded. */
    @Add
    private static native int lastError();

    /**
     * The failure of a call relative to an open directory, which the JDK makes only where
     * {@link #openatSupported} says it may, or where a directory stream it got that way gives the
     * descriptor; neither happens here.
     */
    @Add
    private static UnixException notSupported() {
        return new UnixException(UnixConstants.ENOSYS);
    }

    /** Throws the error of the last call, if it failed. */
    @Add
    private static void check() throws UnixException {
        int errno = lastError();
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    @Replace
    static byte[] getcwd() {
        return getcwd0();
    }

    @Add
    private static native byte[] getcwd0();

    @Replace
    static int dup(int filedes) throws UnixException {
        int fd = dup0(filedes);
        check();
        return fd;
    }

    @Add
    private static native int dup0(int filedes);

    @Replace
    static int open(UnixPath path, int flags, int mode) throws UnixException {
        int fd = open0(path.getByteArrayForSysCalls(), flags, mode);
        check();
        return fd;
    }

    @Add
    private static native int open0(byte[] path, int flags, int mode);

    @Replace
    static int openat(int dfd, byte[] path, int flags, int mode) throws UnixException {
        throw notSupported();
    }

    @Replace
    static void close(int fd) throws UnixException {
        if (fd != -1) {
            close0(fd);
            check();
        }
    }

    @Replace
    private static native void close0(int fd);

    @Replace
    static void link(UnixPath existing, UnixPath newfile) throws UnixException {
        link0(existing.getByteArrayForSysCalls(), newfile.getByteArrayForSysCalls());
        check();
    }

    @Add
    private static native void link0(byte[] existing, byte[] newfile);

    @Replace
    static void unlink(UnixPath path) throws UnixException {
        unlink0(path.getByteArrayForSysCalls());
        check();
    }

    @Add
    private static native void unlink0(byte[] path);

    @Replace
    static void unlinkat(int dfd, byte[] path, int flag) throws UnixException {
        throw notSupported();
    }

    @Replace
    static void mknod(UnixPath path, int mode, long dev) throws UnixException {
        mknod0(path.getByteArrayForSysCalls(), mode, dev);
    }

    @Add
    private static native void mknod0(byte[] path, int mode, long dev) throws UnixException;

    @Replace
    static void rename(UnixPath from, UnixPath to) throws UnixException {
        rename0(from.getByteArrayForSysCalls(), to.getByteArrayForSysCalls());
        check();
    }

    @Add
    private static native void rename0(byte[] from, byte[] to);

    @Replace
    static void renameat(int fromfd, byte[] from, int tofd, byte[] to) throws UnixException {
        throw notSupported();
    }

    @Replace
    static void mkdir(UnixPath path, int mode) throws UnixException {
        mkdir0(path.getByteArrayForSysCalls(), mode);
        check();
    }

    @Add
    private static native void mkdir0(byte[] path, int mode);

    @Replace
    static void rmdir(UnixPath path) throws UnixException {
        rmdir0(path.getByteArrayForSysCalls());
        check();
    }

    @Add
    private static native void rmdir0(byte[] path);

    @Replace
    static byte[] readlink(UnixPath path) throws UnixException {
        byte[] target = readlink0(path.getByteArrayForSysCalls());
        check();
        return target;
    }

    @Add
    private static native byte[] readlink0(byte[] path);

    @Replace
    static byte[] realpath(UnixPath path) throws UnixException {
        byte[] real = realpath0(path.getByteArrayForSysCalls());
        check();
        return real;
    }

    @Add
    private static native byte[] realpath0(byte[] path);

    @Replace
    static void symlink(byte[] name1, UnixPath name2) throws UnixException {
        symlink0(name1, name2.getByteArrayForSysCalls());
        check();
    }

    @Add
    private static native void symlink0(byte[] name1, byte[] name2);

    /** {@code stat}, or {@code lstat} where {@code flag} is {@code AT_SYMLINK_NOFOLLOW}: the errno. */
    @Add
    private static int stat(byte[] path, int flag, UnixFileAttributes attrs) {
        long[] fields = new long[STAT_FIELDS];
        stat0(path, flag, fields);
        int errno = lastError();
        if (errno == 0) {
            attrs.fill(fields);
        }
        return errno;
    }

    @Add
    private static native void stat0(byte[] path, int flag, long[] fields);

    @Replace
    static void stat(UnixPath path, UnixFileAttributes attrs) throws UnixException {
        int errno = stat2(path, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    @Replace
    static int stat2(UnixPath path, UnixFileAttributes attrs) {
        return stat(path.getByteArrayForSysCalls(), 0, attrs);
    }

    @Replace
    static void lstat(UnixPath path, UnixFileAttributes attrs) throws UnixException {
        int errno = stat(path.getByteArrayForSysCalls(), UnixConstants.AT_SYMLINK_NOFOLLOW, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    @Replace
    static void fstat(int fd, UnixFileAttributes attrs) throws UnixException {
        long[] fields = new long[STAT_FIELDS];
        fstat0(fd, fields);
        check();
        attrs.fill(fields);
    }

    @Add
    private static native void fstat0(int fd, long[] fields);

    @Replace
    static void fstatat(int dfd, UnixPath path, int flag, UnixFileAttributes attrs)
        throws UnixException
    {
        int errno = fstatat2(dfd, path, flag, attrs);
        if (errno != 0) {
            throw new UnixException(errno);
        }
    }

    /** {@code fstatat} relative to the working directory, which is {@code stat} or {@code lstat}. */
    @Replace
    static int fstatat2(int dfd, UnixPath path, int flag, UnixFileAttributes attrs) {
        if (dfd != UnixConstants.AT_FDCWD) {
            return UnixConstants.ENOSYS;
        }
        return stat(path.getByteArrayForSysCalls(), flag, attrs);
    }

    @Replace
    static void chown(UnixPath path, int uid, int gid) throws UnixException {
        chown0(path.getByteArrayForSysCalls(), uid, gid, 0);
        check();
    }

    @Add
    private static native void chown0(byte[] path, int uid, int gid, int flag);

    @Replace
    static void lchown(UnixPath path, int uid, int gid) throws UnixException {
        chown0(path.getByteArrayForSysCalls(), uid, gid, UnixConstants.AT_SYMLINK_NOFOLLOW);
        check();
    }

    @Replace
    static void fchown(int fd, int uid, int gid) throws UnixException {
        fchown0(fd, uid, gid);
        check();
    }

    @Replace
    private static native void fchown0(int fd, int uid, int gid);

    @Replace
    static void chmod(UnixPath path, int mode) throws UnixException {
        chmod0(path.getByteArrayForSysCalls(), mode);
        check();
    }

    @Add
    private static native void chmod0(byte[] path, int mode);

    @Replace
    static void fchmod(int fd, int mode) throws UnixException {
        fchmod0(fd, mode);
        check();
    }

    @Replace
    private static native void fchmod0(int fd, int mode);

    @Replace
    static void fchmodat(int fd, UnixPath path, int mode, int flag) throws UnixException {
        throw notSupported();
    }

    @Replace
    static void futimens(int fd, long times0, long times1) throws UnixException {
        futimens0(fd, times0, times1);
        check();
    }

    @Replace
    private static native void futimens0(int fd, long times0, long times1);

    @Replace
    static void utimensat(int fd, UnixPath path, long times0, long times1, int flags)
        throws UnixException
    {
        if (fd != UnixConstants.AT_FDCWD) {
            throw notSupported();
        }
        utimensat0(path.getByteArrayForSysCalls(), times0, times1, flags);
        check();
    }

    @Add
    private static native void utimensat0(byte[] path, long times0, long times1, int flags);

    @Replace
    static long opendir(UnixPath path) throws UnixException {
        long dir = opendir0(path.getByteArrayForSysCalls());
        check();
        return dir;
    }

    @Add
    private static native long opendir0(byte[] path);

    @Replace
    static long fdopendir(int dfd) throws UnixException {
        throw notSupported();
    }

    @Replace
    static void closedir(long dir) throws UnixException {
        closedir0(dir);
        check();
    }

    @Add
    private static native void closedir0(long dir);

    /** The name of the next entry of the directory, or null after the last. */
    @Replace
    static byte[] readdir(long dir) throws UnixException {
        byte[] name = readdir0(dir);
        check();
        return name.length == 0 ? null : name;
    }

    @Replace
    private static native byte[] readdir0(long dir);

    /** {@code access(path, amode)}: 0, or the errno of why not. */
    @Replace
    static int access(UnixPath path, int amode) {
        return access0(path.getByteArrayForSysCalls(), amode);
    }

    @Add
    private static native int access0(byte[] path, int amode);

    /** The name of the user {@code uid}, or a {@code UnixException} with errno 0 where there is none. */
    @Replace
    static byte[] getpwuid(int uid) throws UnixException {
        byte[] name = getpwuid0(uid);
        if (name.length == 0) {
            throw new UnixException(lastError());
        }
        return name;
    }

    @Add
    private static native byte[] getpwuid0(int uid);

    /** The name of the group {@code gid}, or a {@code UnixException} with errno 0 where there is none. */
    @Replace
    static byte[] getgrgid(int gid) throws UnixException {
        byte[] name = getgrgid0(gid);
        if (name.length == 0) {
            throw new UnixException(lastError());
        }
        return name;
    }

    @Add
    private static native byte[] getgrgid0(int gid);

    @Replace
    static int getpwnam(String name) throws UnixException {
        return getpwnam0(Util.toBytes(name));
    }

    @Add
    private static native int getpwnam0(byte[] name) throws UnixException;

    @Replace
    static int getgrnam(String name) throws UnixException {
        return getgrnam0(Util.toBytes(name));
    }

    @Add
    private static native int getgrnam0(byte[] name) throws UnixException;

    @Replace
    static void statvfs(UnixPath path, UnixFileStoreAttributes attrs) throws UnixException {
        statvfs0(path.getByteArrayForSysCalls(), attrs);
    }

    @Add
    private static native void statvfs0(byte[] path, UnixFileStoreAttributes attrs) throws UnixException;

    @Replace
    static byte[] strerror(int errnum) {
        return strerror0(errnum);
    }

    @Add
    private static native byte[] strerror0(int errnum);

    @Replace
    static int fgetxattr(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException
    {
        return fgetxattr0(filedes, name, valueAddress, valueLen);
    }

    @Add
    private static native int fgetxattr0(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException;

    @Replace
    static void fsetxattr(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException
    {
        fsetxattr0(filedes, name, valueAddress, valueLen);
    }

    @Add
    private static native void fsetxattr0(int filedes, byte[] name, long valueAddress, int valueLen)
        throws UnixException;

    @Replace
    static void fremovexattr(int filedes, byte[] name) throws UnixException {
        fremovexattr0(filedes, name);
    }

    @Add
    private static native void fremovexattr0(int filedes, byte[] name) throws UnixException;

    /**
     * The natives of the translator's {@code FileChannel} ({@code UnixFileChannel}), on a
     * descriptor of the runtime's table: they read or write at the descriptor's position, or at
     * {@code position} where it is not negative, and answer -1 where they fail.
     */
    @Add
    static native int readBytes(int fd, byte[] buf, int off, int len, long position);

    @Add
    static native int writeBytes(int fd, byte[] buf, int off, int len, long position);

    @Add
    static native long position(int fd);

    @Add
    static native void seek(int fd, long position);

    @Add
    static native long size(int fd);

    @Add
    static native void truncate(int fd, long size);

    @Add
    static native void sync(int fd, boolean metadata);

    /** Throws the error of the last call of the natives above, if it failed. */
    @Add
    static void checkLast() throws UnixException {
        check();
    }

    // What the system supports, which the JDK's static initializer asks the native library
    // (`init`, which it loads first): here birth times, and not the calls relative to an open
    // directory or extended attributes.
    @Replace
    private static final int capabilities = SUPPORTS_BIRTHTIME;

    @Replace
    static boolean openatSupported() {
        return false;
    }

    @Replace
    static boolean xattrSupported() {
        return false;
    }

    @Replace
    static boolean fchmodatNoFollowSupported() {
        return false;
    }

    @Remove
    static NativeBuffer copyToNativeBuffer(UnixPath path);

    @Remove
    private static native int open0(long pathAddress, int flags, int mode) throws UnixException;

    @Remove
    private static native int openat0(int dfd, long pathAddress, int flags, int mode) throws UnixException;

    @Remove
    private static native void link0(long existingAddress, long newAddress) throws UnixException;

    @Remove
    private static native void unlink0(long pathAddress) throws UnixException;

    @Remove
    private static native void unlinkat0(int dfd, long pathAddress, int flag) throws UnixException;

    @Remove
    private static native void mknod0(long pathAddress, int mode, long dev) throws UnixException;

    @Remove
    private static native void rename0(long fromAddress, long toAddress) throws UnixException;

    @Remove
    private static native void renameat0(int fromfd, long fromAddress, int tofd, long toAddress) throws UnixException;

    @Remove
    private static native void mkdir0(long pathAddress, int mode) throws UnixException;

    @Remove
    private static native void rmdir0(long pathAddress) throws UnixException;

    @Remove
    private static native byte[] readlink0(long pathAddress) throws UnixException;

    @Remove
    private static native byte[] realpath0(long pathAddress) throws UnixException;

    @Remove
    private static native void symlink0(long name1, long name2) throws UnixException;

    @Remove
    private static native int stat0(long pathAddress, UnixFileAttributes attrs);

    @Remove
    private static native void lstat0(long pathAddress, UnixFileAttributes attrs) throws UnixException;

    @Remove
    private static native void fstat0(int fd, UnixFileAttributes attrs) throws UnixException;

    @Remove
    private static native int fstatat0(int dfd, long pathAddress, int flag, UnixFileAttributes attrs);

    @Remove
    private static native void chown0(long pathAddress, int uid, int gid) throws UnixException;

    @Remove
    private static native void lchown0(long pathAddress, int uid, int gid) throws UnixException;

    @Remove
    private static native void chmod0(long pathAddress, int mode) throws UnixException;

    @Remove
    private static native void fchmodat0(int fd, long pathAddress, int mode, int flag) throws UnixException;

    @Remove
    private static native void utimensat0(int fd, long pathAddress, long times0, long times1, int flags) throws UnixException;

    @Remove
    private static native long opendir0(long pathAddress) throws UnixException;

    @Remove
    private static native int access0(long pathAddress, int amode);

    @Remove
    private static native int getpwnam0(long nameAddress) throws UnixException;

    @Remove
    private static native int getgrnam0(long nameAddress) throws UnixException;

    @Remove
    private static native void statvfs0(long pathAddress, UnixFileStoreAttributes attrs) throws UnixException;

    @Remove
    private static native int fgetxattr0(int filedes, long nameAddress, long valueAddress, int valueLen) throws UnixException;

    @Remove
    private static native void fsetxattr0(int filedes, long nameAddress, long valueAddress, int valueLen) throws UnixException;

    @Remove
    private static native void fremovexattr0(int filedes, long nameAddress) throws UnixException;

    @Remove
    private static final int SUPPORTS_OPENAT;

    @Remove
    private static final int SUPPORTS_XATTR;

    @Remove
    private static native boolean fchmodatNoFollowSupported0();

    @Remove
    private static native int init();

    // The JDK's static initializer is left out (see `capabilities`).
    static {}
}
