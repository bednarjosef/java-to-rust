package sun.nio.fs;

import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.FileTime;
import java.nio.file.attribute.GroupPrincipal;
import java.nio.file.attribute.PosixFileAttributes;
import java.nio.file.attribute.PosixFilePermission;
import java.nio.file.attribute.UserPrincipal;
import java.util.EnumSet;
import java.util.Set;
import java.util.concurrent.TimeUnit;

/**
 * The translator's {@code UnixFileAttributes}: the {@code stat} of a file, which replaces the
 * JDK's in a translation with the JDK's sources. The JDK's natives write its fields; here the
 * translator's {@code UnixNativeDispatcher} gives them as an array, which {@link #fill} copies.
 * The rest answers as the JDK's does.
 */
class UnixFileAttributes implements PosixFileAttributes {
    private int st_mode;
    private long st_ino;
    private long st_dev;
    private long st_rdev;
    private int st_nlink;
    private int st_uid;
    private int st_gid;
    private long st_size;
    private long st_atime_sec;
    private long st_atime_nsec;
    private long st_mtime_sec;
    private long st_mtime_nsec;
    private long st_ctime_sec;
    private long st_ctime_nsec;
    private long st_birthtime_sec;
    private long st_birthtime_nsec;
    private boolean birthtime_available;

    private UserPrincipal owner;
    private GroupPrincipal group;
    private UnixFileKey key;

    private UnixFileAttributes() {
    }

    /** The fields in the order of the runtime's {@code stat_fields} ({@code unix.rs}). */
    void fill(long[] f) {
        st_mode = (int) f[0];
        st_ino = f[1];
        st_dev = f[2];
        st_rdev = f[3];
        st_nlink = (int) f[4];
        st_uid = (int) f[5];
        st_gid = (int) f[6];
        st_size = f[7];
        st_atime_sec = f[8];
        st_atime_nsec = f[9];
        st_mtime_sec = f[10];
        st_mtime_nsec = f[11];
        st_ctime_sec = f[12];
        st_ctime_nsec = f[13];
        st_birthtime_sec = f[14];
        st_birthtime_nsec = f[15];
        birthtime_available = f[16] != 0;
    }

    static UnixFileAttributes get(UnixPath path, boolean followLinks) throws UnixException {
        UnixFileAttributes attrs = new UnixFileAttributes();
        if (followLinks) {
            UnixNativeDispatcher.stat(path, attrs);
        } else {
            UnixNativeDispatcher.lstat(path, attrs);
        }
        return attrs;
    }

    static UnixFileAttributes getIfExists(UnixPath path) throws UnixException {
        return getIfExists(path, true);
    }

    /** The attributes of the file, or null where it does not exist. */
    static UnixFileAttributes getIfExists(UnixPath path, boolean followLinks) throws UnixException {
        UnixFileAttributes attrs = new UnixFileAttributes();
        int flag = followLinks ? 0 : UnixConstants.AT_SYMLINK_NOFOLLOW;
        int errno = UnixNativeDispatcher.fstatat2(UnixConstants.AT_FDCWD, path, flag, attrs);
        if (errno == 0) {
            return attrs;
        } else if (errno == UnixConstants.ENOENT) {
            return null;
        } else {
            throw new UnixException(errno);
        }
    }

    static UnixFileAttributes get(int fd) throws UnixException {
        UnixFileAttributes attrs = new UnixFileAttributes();
        UnixNativeDispatcher.fstat(fd, attrs);
        return attrs;
    }

    static UnixFileAttributes get(int dfd, UnixPath path, boolean followLinks) throws UnixException {
        UnixFileAttributes attrs = new UnixFileAttributes();
        int flag = followLinks ? 0 : UnixConstants.AT_SYMLINK_NOFOLLOW;
        UnixNativeDispatcher.fstatat(dfd, path, flag, attrs);
        return attrs;
    }

    boolean isSameFile(UnixFileAttributes attrs) {
        return st_ino == attrs.st_ino && st_dev == attrs.st_dev;
    }

    int mode() { return st_mode; }
    long ino() { return st_ino; }
    long dev() { return st_dev; }
    long rdev() { return st_rdev; }
    int nlink() { return st_nlink; }
    int uid() { return st_uid; }
    int gid() { return st_gid; }

    /** A time in nanoseconds where that fits a {@code long}, else in microseconds, as the JDK's. */
    private static FileTime toFileTime(long sec, long nsec) {
        if (nsec == 0) {
            return FileTime.from(sec, TimeUnit.SECONDS);
        }
        try {
            long nanos = Math.addExact(nsec, Math.multiplyExact(sec, 1_000_000_000L));
            return FileTime.from(nanos, TimeUnit.NANOSECONDS);
        } catch (ArithmeticException ignore) {
            long micro = sec * 1_000_000L + nsec / 1_000L;
            return FileTime.from(micro, TimeUnit.MICROSECONDS);
        }
    }

    FileTime ctime() {
        return toFileTime(st_ctime_sec, st_ctime_nsec);
    }

    boolean isDevice() {
        int type = st_mode & UnixConstants.S_IFMT;
        return type == UnixConstants.S_IFCHR || type == UnixConstants.S_IFBLK || type == UnixConstants.S_IFIFO;
    }

    @Override
    public FileTime lastModifiedTime() {
        return toFileTime(st_mtime_sec, st_mtime_nsec);
    }

    @Override
    public FileTime lastAccessTime() {
        return toFileTime(st_atime_sec, st_atime_nsec);
    }

    /** When the file was created where the system knows it, else when it was last modified. */
    @Override
    public FileTime creationTime() {
        if (UnixNativeDispatcher.birthtimeSupported() && birthtime_available) {
            return toFileTime(st_birthtime_sec, st_birthtime_nsec);
        }
        return lastModifiedTime();
    }

    @Override
    public boolean isRegularFile() {
        return (st_mode & UnixConstants.S_IFMT) == UnixConstants.S_IFREG;
    }

    @Override
    public boolean isDirectory() {
        return (st_mode & UnixConstants.S_IFMT) == UnixConstants.S_IFDIR;
    }

    @Override
    public boolean isSymbolicLink() {
        return (st_mode & UnixConstants.S_IFMT) == UnixConstants.S_IFLNK;
    }

    @Override
    public boolean isOther() {
        int type = st_mode & UnixConstants.S_IFMT;
        return type != UnixConstants.S_IFREG && type != UnixConstants.S_IFDIR && type != UnixConstants.S_IFLNK;
    }

    @Override
    public long size() {
        return st_size;
    }

    @Override
    public synchronized UnixFileKey fileKey() {
        if (key == null) {
            key = new UnixFileKey(st_dev, st_ino);
        }
        return key;
    }

    @Override
    public synchronized UserPrincipal owner() {
        if (owner == null) {
            owner = UnixUserPrincipals.fromUid(st_uid);
        }
        return owner;
    }

    @Override
    public synchronized GroupPrincipal group() {
        if (group == null) {
            group = UnixUserPrincipals.fromGid(st_gid);
        }
        return group;
    }

    @Override
    public Set<PosixFilePermission> permissions() {
        int bits = st_mode & UnixConstants.S_IAMB;
        EnumSet<PosixFilePermission> perms = EnumSet.noneOf(PosixFilePermission.class);
        if ((bits & UnixConstants.S_IRUSR) > 0) perms.add(PosixFilePermission.OWNER_READ);
        if ((bits & UnixConstants.S_IWUSR) > 0) perms.add(PosixFilePermission.OWNER_WRITE);
        if ((bits & UnixConstants.S_IXUSR) > 0) perms.add(PosixFilePermission.OWNER_EXECUTE);
        if ((bits & UnixConstants.S_IRGRP) > 0) perms.add(PosixFilePermission.GROUP_READ);
        if ((bits & UnixConstants.S_IWGRP) > 0) perms.add(PosixFilePermission.GROUP_WRITE);
        if ((bits & UnixConstants.S_IXGRP) > 0) perms.add(PosixFilePermission.GROUP_EXECUTE);
        if ((bits & UnixConstants.S_IROTH) > 0) perms.add(PosixFilePermission.OTHERS_READ);
        if ((bits & UnixConstants.S_IWOTH) > 0) perms.add(PosixFilePermission.OTHERS_WRITE);
        if ((bits & UnixConstants.S_IXOTH) > 0) perms.add(PosixFilePermission.OTHERS_EXECUTE);
        return perms;
    }

    /** The attributes as {@code BasicFileAttributes} only, which do not tell the owner. */
    BasicFileAttributes asBasicFileAttributes() {
        return new Basic(this);
    }

    private static class Basic implements BasicFileAttributes {
        private final UnixFileAttributes attrs;

        Basic(UnixFileAttributes attrs) {
            this.attrs = attrs;
        }

        @Override public FileTime lastModifiedTime() { return attrs.lastModifiedTime(); }
        @Override public FileTime lastAccessTime() { return attrs.lastAccessTime(); }
        @Override public FileTime creationTime() { return attrs.creationTime(); }
        @Override public boolean isRegularFile() { return attrs.isRegularFile(); }
        @Override public boolean isDirectory() { return attrs.isDirectory(); }
        @Override public boolean isSymbolicLink() { return attrs.isSymbolicLink(); }
        @Override public boolean isOther() { return attrs.isOther(); }
        @Override public long size() { return attrs.size(); }
        @Override public Object fileKey() { return attrs.fileKey(); }
    }
}
