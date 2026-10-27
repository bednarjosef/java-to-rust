package sun.management;

/**
 * The translator's patch of the JDK's {@code VMManagementImpl}, the JVM's figures behind
 * {@code java.lang.management} (see docs/jdk-overlays.md).
 *
 * <p>Its natives ask the JVM's management interface. A translated program has no JVM, so the
 * figures a program reads at start (the interface's version, what it supports, the JVM's
 * arguments) are answered here as HotSpot answers them, and the start time and uptime are the
 * process's, from the runtime ({@code ast/natives.rs}). The JVM's own arguments are none: a
 * translated program is started without any.
 */
class VMManagementImpl {
    /** {@code JMM_VERSION} of Java 26's HotSpot, 5.0, as {@code "major.minor"}. */
    @Replace
    private static String getVersion0() {
        return "5.0";
    }

    /** What HotSpot's {@code GetOptionalSupport} says on Linux: all of it. */
    @Replace
    private static void initOptionalSupportFields() {
        compTimeMonitoringSupport = true;
        threadContentionMonitoringSupport = true;
        currentThreadCpuTimeSupport = true;
        otherThreadCpuTimeSupport = true;
        objectMonitorUsageSupport = true;
        synchronizerUsageSupport = true;
        threadAllocatedMemorySupport = true;
        remoteDiagnosticCommandsSupport = true;
    }

    @Replace
    public String[] getVmArguments0() {
        return new String[0];
    }
}
