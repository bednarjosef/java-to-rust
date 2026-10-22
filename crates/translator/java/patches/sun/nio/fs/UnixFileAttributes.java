package sun.nio.fs;

/**
 * The translator's patch of the JDK's {@code UnixFileAttributes}, the {@code stat} of a file (see
 * docs/jdk-overlays.md). The JDK's natives write its fields; here the translator's
 * {@code UnixNativeDispatcher} gives them as an array, which {@link #fill} copies. The owner, the
 * group and the key, made when first asked for, are made under the object's lock instead of
 * through volatile fields.
 */
@Edit(find = "private volatile UserPrincipal owner;", replace = "private UserPrincipal owner;")
@Edit(find = "private volatile GroupPrincipal group;", replace = "private GroupPrincipal group;")
@Edit(find = "private volatile UnixFileKey key;", replace = "private UnixFileKey key;")
class UnixFileAttributes {
    /** The fields in the order of the runtime's {@code stat_fields} ({@code unix.rs}). */
    @Add
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

    @Replace
    @Override
    public synchronized UnixFileKey fileKey() {
        if (key == null) {
            key = new UnixFileKey(st_dev, st_ino);
        }
        return key;
    }

    @Replace
    @Override
    public synchronized UserPrincipal owner() {
        if (owner == null) {
            owner = UnixUserPrincipals.fromUid(st_uid);
        }
        return owner;
    }

    @Replace
    @Override
    public synchronized GroupPrincipal group() {
        if (group == null) {
            group = UnixUserPrincipals.fromGid(st_gid);
        }
        return group;
    }
}
