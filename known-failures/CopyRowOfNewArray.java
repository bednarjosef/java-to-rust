public class CopyRowOfNewArray {
    static class Cell { int row; Cell(int row) { this.row = row; } }
    public static void main(String[] args) {
        Cell[][] grid = new Cell[2][2];
        grid[0][0] = new Cell(4);
        Cell[][] copy = new Cell[2][];
        copy[0] = grid[0];
        System.out.println(copy[0][0].row);
    }
}
