package java.lang;

import java.io.IOException;

/**
 * The translator's patch of the JDK's {@code ProcessImpl} on Linux (see docs/jdk-overlays.md).
 * The runtime starts a process with Rust's {@code std::process} ({@code spawn0}, the runtime's
 * {@code process.rs}), with no launcher of the JDK's in between: the launch mechanism and the
 * path of {@code jspawnhelper} are not used. Everything else, the streams over the pipes, the
 * thread that waits for the process and {@code waitFor}, is the JDK's.
 */
class ProcessImpl {
    /**
     * The JDK's native, with the arrays that may be null given as empty ones: the environment,
     * where the child inherits the parent's ({@code envc} then -1), and the directory, where it
     * runs in the parent's.
     */
    @Replace
    private int forkAndExec(int mode, byte[] helperpath,
                            byte[] prog,
                            byte[] argBlock, int argc,
                            byte[] envBlock, int envc,
                            byte[] dir,
                            int[] fds,
                            boolean redirectErrorStream)
            throws IOException {
        byte[] none = new byte[0];
        return spawn0(prog, argBlock, argc, envBlock == null ? none : envBlock,
                envBlock == null ? -1 : envc, dir == null ? none : dir, fds, redirectErrorStream);
    }

    /**
     * Starts {@code prog} as {@code forkAndExec} describes, and the process's id; an
     * {@code IOException} where it cannot start.
     */
    @Add
    private static native int spawn0(byte[] prog, byte[] argBlock, int argc, byte[] envBlock, int envc,
            byte[] dir, int[] fds, boolean redirectErrorStream) throws IOException;
}
