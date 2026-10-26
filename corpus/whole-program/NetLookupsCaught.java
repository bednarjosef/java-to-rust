import java.net.InetAddress;

// The loopback address, which asks whether an interface has the IPv6 one, and the name of an
// address, looked up where a catch prints whatever they throw.
public class NetLookupsCaught {
    public static void main(String[] args) {
        try {
            InetAddress loopback = InetAddress.getLoopbackAddress();
            System.out.println("loopback " + loopback.isLoopbackAddress());
            System.out.println(InetAddress.getByName("127.0.0.1").getHostName());
        } catch (Throwable t) {
            System.out.println("failed: " + t);
        }
    }
}
