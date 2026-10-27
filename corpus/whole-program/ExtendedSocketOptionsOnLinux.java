import java.io.IOException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.SocketOption;
import java.net.StandardSocketOptions;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.util.Set;
import java.util.TreeSet;
import jdk.net.ExtendedSocketOptions;

/**
 * The socket options of `jdk.net` on Linux, as a server's network code asks for them: which
 * options a socket and a channel support (the question Netty asks of each channel), and the
 * keep-alive and quick-ack options set and read back on a connection over the
 * loopback address, with the errors of bad values and of a closed socket.
 */
public class ExtendedSocketOptionsOnLinux {
    public static void main(String[] args) throws Exception {
        InetAddress loopback = InetAddress.getLoopbackAddress();
        try (ServerSocket server = new ServerSocket(0, 50, loopback);
                Socket client = new Socket(loopback, server.getLocalPort());
                Socket accepted = server.accept()) {
            System.out.println("socket options: " + names(client.supportedOptions()));
            System.out.println("server options: " + names(server.supportedOptions()));
            keepAlive(client);
            quickAck(accepted);
            dontFragment(client);
            System.out.println("napi id: " + (client.getOption(ExtendedSocketOptions.SO_INCOMING_NAPI_ID) >= 0));
            errors(client);
        }
        try (ServerSocketChannel server = ServerSocketChannel.open().bind(new InetSocketAddress(loopback, 0));
                SocketChannel client = SocketChannel.open(server.getLocalAddress())) {
            System.out.println("channel options: " + names(client.supportedOptions()));
            client.setOption(ExtendedSocketOptions.TCP_KEEPIDLE, 77);
            client.setOption(ExtendedSocketOptions.TCP_KEEPINTERVAL, 11);
            System.out.println("channel keep-alive: " + client.getOption(ExtendedSocketOptions.TCP_KEEPIDLE) + " "
                    + client.getOption(ExtendedSocketOptions.TCP_KEEPINTERVAL));
        }
        Socket closed = new Socket();
        closed.close();
        try {
            closed.setOption(ExtendedSocketOptions.TCP_KEEPCOUNT, 3);
        } catch (IOException e) {
            System.out.println("closed: " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    static String names(Set<SocketOption<?>> options) {
        Set<String> names = new TreeSet<>();
        for (SocketOption<?> option : options) {
            names.add(option.name());
        }
        return names.toString();
    }

    static void keepAlive(Socket socket) throws IOException {
        socket.setOption(StandardSocketOptions.SO_KEEPALIVE, true);
        socket.setOption(ExtendedSocketOptions.TCP_KEEPIDLE, 120);
        socket.setOption(ExtendedSocketOptions.TCP_KEEPINTERVAL, 15);
        socket.setOption(ExtendedSocketOptions.TCP_KEEPCOUNT, 4);
        System.out.println("keep-alive: " + socket.getOption(ExtendedSocketOptions.TCP_KEEPIDLE) + " "
                + socket.getOption(ExtendedSocketOptions.TCP_KEEPINTERVAL) + " "
                + socket.getOption(ExtendedSocketOptions.TCP_KEEPCOUNT));
    }

    static void quickAck(Socket socket) throws IOException {
        socket.setOption(ExtendedSocketOptions.TCP_QUICKACK, true);
        // The kernel turns quick acks off again by itself, so only that the read works is printed.
        Boolean on = socket.getOption(ExtendedSocketOptions.TCP_QUICKACK);
        System.out.println("quick ack read: " + (on != null));
    }

    static void dontFragment(Socket socket) throws IOException {
        // Only datagram sockets have it.
        try {
            socket.setOption(ExtendedSocketOptions.IP_DONTFRAGMENT, true);
        } catch (UnsupportedOperationException e) {
            System.out.println("don't fragment: " + e.getMessage());
        }
    }

    static void errors(Socket socket) throws IOException {
        try {
            socket.setOption(ExtendedSocketOptions.TCP_KEEPIDLE, -1);
        } catch (IOException | IllegalArgumentException e) {
            System.out.println("negative idle: " + e.getClass().getName() + ": " + e.getMessage());
        }
        try {
            socket.setOption(ExtendedSocketOptions.TCP_KEEPCOUNT, 0);
        } catch (IOException | IllegalArgumentException e) {
            System.out.println("zero count: " + e.getClass().getName() + ": " + e.getMessage());
        }
        try {
            socket.setOption(ExtendedSocketOptions.TCP_KEEPINTERVAL, 1 << 20);
        } catch (IOException | IllegalArgumentException e) {
            System.out.println("huge interval: " + e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
