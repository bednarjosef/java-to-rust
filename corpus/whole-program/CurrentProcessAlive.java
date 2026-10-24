// The current process through `ProcessHandle`: its id, and that it is alive.
public class CurrentProcessAlive {
    public static void main(String[] args) {
        ProcessHandle self = ProcessHandle.current();
        System.out.println(self.pid() > 0);
        System.out.println(self.isAlive());
    }
}
