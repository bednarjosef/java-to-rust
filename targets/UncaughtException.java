// Features: a custom unchecked exception that escapes main, so the program exits with code 1
// after printing; a state machine validating a sequence of commands, exception with a formatted
// message, an enum of states, a switch expression, and a finally block that still prints before
// the exception leaves the program. The expected exit code is 1 (see UncaughtException.exitcode).
import java.util.List;

public class UncaughtException {
    static class ProtocolViolation extends RuntimeException {
        ProtocolViolation(String message) {
            super(message);
        }
    }

    enum State {
        IDLE, CONNECTING, CONNECTED, CLOSED
    }

    static State next(State current, String command) {
        return switch (current) {
            case IDLE -> switch (command) {
                case "connect" -> State.CONNECTING;
                default -> throw new ProtocolViolation("cannot " + command + " while " + current);
            };
            case CONNECTING -> command.equals("ack") ? State.CONNECTED : fail(current, command);
            case CONNECTED -> switch (command) {
                case "send" -> State.CONNECTED;
                case "close" -> State.CLOSED;
                default -> fail(current, command);
            };
            case CLOSED -> fail(current, command);
        };
    }

    static State fail(State state, String command) {
        throw new ProtocolViolation(String.format("command '%s' not allowed in %s", command, state));
    }

    static State run(List<String> commands) {
        State state = State.IDLE;
        int step = 0;
        try {
            for (String command : commands) {
                state = next(state, command);
                step++;
                System.out.println("step " + step + ": " + command + " -> " + state);
            }
        } finally {
            System.out.println("finished after " + step + " steps in " + state);
        }
        return state;
    }

    public static void main(String[] args) {
        State ok = run(List.of("connect", "ack", "send", "send", "close"));
        System.out.println("first session ended " + ok + " (ordinal " + ok.ordinal() + ")");

        System.out.println("second session:");
        run(List.of("connect", "ack", "send", "connect"));
        System.out.println("never printed");
    }
}
