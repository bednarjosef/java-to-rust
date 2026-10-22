import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.UnknownHostException;
import java.util.Arrays;

// Names and addresses through the system's name service, as InetAddress looks them up: literal
// addresses, the loopback address, localhost (from the hosts file), a name that is not found,
// the name of an address, and this host's own name. No connection is made.
public class NameLookups {
    static String describe(InetAddress a) {
        return a + " " + (a instanceof Inet6Address ? "v6" : "v4") + " loopback=" + a.isLoopbackAddress()
                + " " + Arrays.toString(a.getAddress());
    }

    public static void main(String[] args) throws UnknownHostException {
        System.out.println(describe(InetAddress.getByName("127.0.0.1")));
        System.out.println(describe(InetAddress.getByName("::1")));
        System.out.println(describe(InetAddress.getByAddress("example", new byte[] {10, 0, 0, 1})));
        System.out.println(describe(InetAddress.getLoopbackAddress()));
        for (InetAddress a : InetAddress.getAllByName("localhost")) {
            System.out.println(describe(a) + " " + a.getHostName());
        }
        InetAddress first = InetAddress.getByName("localhost");
        System.out.println(first.equals(InetAddress.getByName("localhost")) + " " + first.getHostAddress());
        try {
            InetAddress.getByName("no-such-host.invalid");
        } catch (UnknownHostException e) {
            System.out.println("unknown: " + e.getMessage());
        }
        System.out.println(InetAddress.getByName("127.0.0.1").getHostName());
        System.out.println(InetAddress.getByName("127.0.0.1").getCanonicalHostName());
        try {
            InetAddress local = InetAddress.getLocalHost();
            System.out.println("local host has a name: " + !local.getHostName().isEmpty());
        } catch (UnknownHostException e) {
            System.out.println("local host unknown");
        }
    }
}
