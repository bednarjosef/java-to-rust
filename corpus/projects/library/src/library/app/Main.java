package library.app;

import library.desk.Loans;
import library.items.Book;
import library.items.Disc;
import library.items.Item;
import library.items.Room;

public class Main {
    public static void main(String[] args) {
        Loans loans = new Loans();
        Book small = new Book("Notes", 120);
        Book big = new Book("Atlas", 640);
        Disc disc = new Disc("Waves");
        loans.lend(small, 20);
        loans.lend(big, 20);
        loans.lend(disc, 9);
        loans.lend(Room.GROUP, 2);
        disc.play();
        disc.play();
        System.out.print(loans.report());
        System.out.println("fines " + loans.fines());
        System.out.println(small.compareTo(big) + " " + big.compareTo(small) + " " + small.compareTo(small));
        System.out.println(loans.has(big) + " " + loans.has(new Book("Atlas", 640)) + " " + loans.has(Room.GROUP)
                + " " + loans.has(Room.QUIET));
        System.out.println(Item.LOAN_DAYS + " " + Item.fine(Room.QUIET, 5));
        System.out.println(loans.bookPages() + " pages, " + loans.discs() + " disc, first " + loans.first().title());
    }
}
