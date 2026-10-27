import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.InterfaceAddress;
import java.net.NetworkInterface;
import java.util.Collections;
import java.util.List;

/**
 * The network interfaces as Netty and log4j ask about them: the loopback interface found among
 * all of them by its address (Netty's `NetUtil`), its flags, MTU, addresses and their prefixes,
 * the lookups by name, index and address, and every interface's hardware address, which is six
 * bytes or none. Only the loopback interface's details are printed, as the others differ between
 * machines.
 */
public class LoopbackInterface {
    public static void main(String[] args) throws Exception {
        List<NetworkInterface> all = Collections.list(NetworkInterface.getNetworkInterfaces());
        NetworkInterface loopback = null;
        for (NetworkInterface nif : all) {
            for (InetAddress address : Collections.list(nif.getInetAddresses())) {
                if (address.isLoopbackAddress() && loopback == null) {
                    loopback = nif;
                }
            }
        }
        System.out.println("loopback: " + loopback);
        System.out.println("name " + loopback.getName() + ", display name " + loopback.getDisplayName());
        System.out.println("index > 0: " + (loopback.getIndex() > 0));
        System.out.println("loopback " + loopback.isLoopback() + ", up " + loopback.isUp() + ", point to point "
                + loopback.isPointToPoint() + ", virtual " + loopback.isVirtual() + ", multicast "
                + loopback.supportsMulticast());
        System.out.println("mtu: " + loopback.getMTU());
        System.out.println("hardware address: " + loopback.getHardwareAddress());
        System.out.println("parent: " + loopback.getParent() + ", children: "
                + Collections.list(loopback.getSubInterfaces()));
        for (InetAddress address : Collections.list(loopback.getInetAddresses())) {
            System.out.println("  address " + address + (address instanceof Inet4Address ? " (IPv4)" : " (IPv6)"));
        }
        for (InterfaceAddress binding : loopback.getInterfaceAddresses()) {
            System.out.println("  binding " + binding.getAddress() + "/" + binding.getNetworkPrefixLength()
                    + " broadcast " + binding.getBroadcast());
        }
        InetAddress local = InetAddress.getByAddress(new byte[] {127, 0, 0, 1});
        System.out.println("by address: " + NetworkInterface.getByInetAddress(local).getName());
        System.out.println("by index: " + NetworkInterface.getByIndex(loopback.getIndex()).equals(loopback));
        System.out.println("by name: " + NetworkInterface.getByName(loopback.getName()).getIndex()
                + " == " + loopback.getIndex());
        System.out.println("no such name: " + NetworkInterface.getByName("no-such-interface"));
        boolean consistent = true;
        for (NetworkInterface nif : all) {
            byte[] hardware = nif.getHardwareAddress();
            consistent &= hardware == null || hardware.length == 6;
            consistent &= NetworkInterface.getByName(nif.getName()).getIndex() == nif.getIndex();
            consistent &= nif.getInterfaceAddresses().size() == Collections.list(nif.getInetAddresses()).size();
            nif.isUp();
            nif.getMTU();
        }
        System.out.println("every interface consistent: " + consistent + ", one of them the loopback: "
                + all.contains(loopback));
        try {
            System.out.println("an address of no interface: " + NetworkInterface.getByInetAddress(
                    InetAddress.getByAddress(new byte[] {(byte) 192, 0, 2, 77})));
        } catch (Exception e) {
            System.out.println(e);
        }
    }
}
