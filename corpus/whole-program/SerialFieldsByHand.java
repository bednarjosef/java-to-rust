import java.io.IOException;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.Serializable;

// Serialization's own types, which a translation does not have: an `ObjectInputStream.GetField`
// and an `ObjectOutputStream.PutField` are only ever null there, as no stream exists to make one.
public class SerialFieldsByHand {
    static class Point implements Serializable {
        int x;
        int y;

        private void readObject(ObjectInputStream s) throws IOException, ClassNotFoundException {
            ObjectInputStream.GetField fields = s.readFields();
            x = fields.get("x", 0);
            y = fields.get("y", 0);
        }

        private void writeObject(ObjectOutputStream s) throws IOException {
            ObjectOutputStream.PutField fields = s.putFields();
            fields.put("x", x);
            fields.put("y", y);
            s.writeFields();
        }
    }

    public static void main(String[] args) throws Exception {
        Point p = new Point();
        p.x = 3;
        try {
            p.readObject(null);
        } catch (NullPointerException e) {
            System.out.println("no stream to read from");
        }
        try {
            p.writeObject(null);
        } catch (NullPointerException e) {
            System.out.println("no stream to write to");
        }
        ObjectInputStream.GetField none = null;
        System.out.println(p.x + " " + p.y + " " + (none == null));
    }
}
