import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.StandardSocketOptions;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.util.Iterator;

/**
 * Connections on the loopback address, as a server's network code makes them: a non-blocking
 * server channel and its selector (the shape of Netty's NIO transport), gathering writes and
 * scattering reads, the blocking `ServerSocket` and `Socket` with a thread on each side, socket
 * options, and the errors of a refused connection and of a port already bound.
 */
public class LocalSocketsAndSelectors {
    public static void main(String[] args) throws Exception {
        selected();
        blocking();
        errors();
    }

    static void selected() throws IOException {
        InetAddress loopback = InetAddress.getLoopbackAddress();
        try (Selector selector = Selector.open();
                ServerSocketChannel server = ServerSocketChannel.open()) {
            server.setOption(StandardSocketOptions.SO_REUSEADDR, true);
            server.bind(new InetSocketAddress(loopback, 0), 50);
            server.configureBlocking(false);
            server.register(selector, SelectionKey.OP_ACCEPT);
            InetSocketAddress bound = (InetSocketAddress) server.getLocalAddress();
            System.out.println("bound to loopback: " + bound.getAddress().isLoopbackAddress() + ", port > 0: "
                    + (bound.getPort() > 0));
            System.out.println("reuse address: " + server.getOption(StandardSocketOptions.SO_REUSEADDR));

            SocketChannel client = SocketChannel.open();
            client.configureBlocking(false);
            boolean connected = client.connect(bound);
            client.register(selector, SelectionKey.OP_CONNECT);

            SocketChannel accepted = null;
            StringBuilder received = new StringBuilder();
            StringBuilder accepting = new StringBuilder();
            boolean madeConnection = false;
            boolean sent = false;
            long writtenCount = 0;
            int rounds = 0;
            while (received.length() < 11 && rounds++ < 100) {
                selector.select(1000);
                Iterator<SelectionKey> keys = selector.selectedKeys().iterator();
                while (keys.hasNext()) {
                    SelectionKey key = keys.next();
                    keys.remove();
                    if (key.isAcceptable()) {
                        accepted = server.accept();
                        accepted.configureBlocking(false);
                        accepted.setOption(StandardSocketOptions.TCP_NODELAY, true);
                        accepted.register(selector, SelectionKey.OP_READ);
                        accepting.append("accepted from loopback: ")
                                .append(((InetSocketAddress) accepted.getRemoteAddress()).getAddress().isLoopbackAddress())
                                .append("\nno delay: ")
                                .append(accepted.getOption(StandardSocketOptions.TCP_NODELAY));
                    } else if (key.isConnectable()) {
                        SocketChannel channel = (SocketChannel) key.channel();
                        if (connected || channel.finishConnect()) {
                            key.interestOps(SelectionKey.OP_WRITE);
                            madeConnection = true;
                        }
                    } else if (key.isWritable() && !sent) {
                        ByteBuffer[] parts = {
                            ByteBuffer.wrap("hello".getBytes(StandardCharsets.UTF_8)),
                            ByteBuffer.wrap(", ".getBytes(StandardCharsets.UTF_8)),
                            ByteBuffer.wrap("nio".getBytes(StandardCharsets.UTF_8)),
                        };
                        long written = client.write(parts);
                        ByteBuffer rest = ByteBuffer.allocateDirect(1);
                        rest.put((byte) '!').flip();
                        written += client.write(rest);
                        writtenCount = written;
                        sent = true;
                        key.interestOps(0);
                    } else if (key.isReadable()) {
                        ByteBuffer first = ByteBuffer.allocate(4);
                        ByteBuffer second = ByteBuffer.allocateDirect(16);
                        long n = ((SocketChannel) key.channel()).read(new ByteBuffer[] {first, second});
                        if (n > 0) {
                            first.flip();
                            second.flip();
                            received.append(StandardCharsets.UTF_8.decode(first));
                            received.append(StandardCharsets.UTF_8.decode(second));
                        }
                    }
                }
            }
            System.out.println(accepting);
            System.out.println("connected: " + madeConnection);
            System.out.println("written: " + writtenCount);
            System.out.println("received: " + received);
            client.close();
            ByteBuffer end = ByteBuffer.allocate(8);
            accepted.keyFor(selector).cancel();
            selector.selectNow();
            accepted.configureBlocking(true);
            System.out.println("read after the peer closed: " + accepted.read(end));
            accepted.close();
            selector.wakeup();
            System.out.println("selected after a wakeup: " + selector.select());
        }
    }

    static void blocking() throws Exception {
        try (ServerSocket server = new ServerSocket(0, 50, InetAddress.getLoopbackAddress())) {
            int port = server.getLocalPort();
            Thread peer = new Thread(() -> {
                try (Socket socket = new Socket(InetAddress.getLoopbackAddress(), port)) {
                    OutputStream out = socket.getOutputStream();
                    out.write("ping\n".getBytes(StandardCharsets.UTF_8));
                    out.flush();
                    InputStream in = socket.getInputStream();
                    byte[] reply = in.readAllBytes();
                    System.out.println("client got: " + new String(reply, StandardCharsets.UTF_8).trim());
                } catch (IOException e) {
                    System.out.println("client failed: " + e);
                }
            });
            peer.start();
            try (Socket socket = server.accept()) {
                InputStream in = socket.getInputStream();
                StringBuilder line = new StringBuilder();
                int c;
                while ((c = in.read()) != -1 && c != '\n') {
                    line.append((char) c);
                }
                System.out.println("server got: " + line);
                socket.getOutputStream().write("pong\n".getBytes(StandardCharsets.UTF_8));
                socket.shutdownOutput();
            }
            peer.join();
        }
    }

    static void errors() throws IOException {
        int port;
        try (ServerSocket server = new ServerSocket(0, 50, InetAddress.getLoopbackAddress())) {
            port = server.getLocalPort();
            try (ServerSocket again = new ServerSocket()) {
                again.bind(new InetSocketAddress(InetAddress.getLoopbackAddress(), port));
                System.out.println("bound twice");
            } catch (IOException e) {
                System.out.println("second bind: " + e.getClass().getName() + ": " + e.getMessage());
            }
        }
        try (SocketChannel channel = SocketChannel.open()) {
            channel.connect(new InetSocketAddress(InetAddress.getLoopbackAddress(), port));
            System.out.println("connected to a closed port");
        } catch (IOException e) {
            System.out.println("connect: " + e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
