// A generic method of an abstract class that no object of the program is of, as the JDK's
// `DatagramSocketImpl.setOption` is in a program that makes no datagram sockets: the method stays
// generic, also in the dispatcher that a translation split into crates gives the class, and its
// call on a field that stays null throws.
public class GenericMethodWithoutObjects {
    static final class Option<T> {
        final String name;

        Option(String name) {
            this.name = name;
        }
    }

    abstract static class Socket {
        <T> void setOption(Option<T> name, T value) {
            System.out.println("set " + name.name);
        }
    }

    static final class Plain extends Socket {
        @Override
        <T> void setOption(Option<T> name, T value) {
            System.out.println("plain " + name.name);
        }
    }

    static Socket socket;

    public static void main(String[] args) {
        System.out.println(socket == null);
        try {
            socket.setOption(new Option<String>("timeout"), "30");
        } catch (NullPointerException e) {
            System.out.println("no socket");
        }
    }
}
