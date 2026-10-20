public class StringBuffers {
    public static void main(String[] args) {
        StringBuffer sb = new StringBuffer();
        System.out.println(sb.length() + " " + sb.isEmpty());
        sb.append("abc").append(1).append('x').append(2.5).append(true);
        System.out.println(sb.toString() + " " + sb.length());
        sb.insert(0, "[").insert(sb.length(), ']');
        System.out.println(sb);
        sb.reverse();
        System.out.println(sb.toString());
        sb.setCharAt(0, 'Q');
        System.out.println(sb.charAt(0) + " " + sb.indexOf("x") + " " + sb.lastIndexOf("1"));
        sb.deleteCharAt(1).delete(0, 2).replace(0, 3, "Z");
        System.out.println(sb.substring(2) + " " + sb.substring(0, 3));
        sb.setLength(4);
        System.out.println("[" + sb + "]");

        StringBuffer copy = new StringBuffer("start");
        StringBuffer sized = new StringBuffer(64);
        sized.append(copy).append(copy.compareTo(sized) < 0);
        System.out.println(sized.toString() + " " + copy.compareTo(new StringBuffer("star")));
        System.out.println(String.valueOf(copy) + " " + copy.toString().equals("start"));

        StringBuilder builder = new StringBuilder(new StringBuffer("from buffer"));
        System.out.println(builder.reverse());
        StringBuffer buffer = new StringBuffer(builder);
        buffer.append(buffer.length());
        System.out.println(buffer);
    }
}
