import java.lang.constant.ConstantDescs;
import java.lang.constant.MethodTypeDesc;

/**
 * Descriptors of method types. The JDK builds a setter's descriptor with
 * {@code ConstantUtils.concat}, which needs the JVM's {@code JavaLangAccess.concat}.
 */
public class MethodTypeDescs {
    public static void main(String[] args) {
        MethodTypeDesc setter = MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_int);
        System.out.println(setter.descriptorString());

        MethodTypeDesc stringSetter = MethodTypeDesc.of(ConstantDescs.CD_void, ConstantDescs.CD_String);
        System.out.println(stringSetter.descriptorString());

        MethodTypeDesc getter = MethodTypeDesc.of(ConstantDescs.CD_long);
        System.out.println(getter.descriptorString());

        MethodTypeDesc many = MethodTypeDesc.of(ConstantDescs.CD_String, ConstantDescs.CD_int, ConstantDescs.CD_String);
        System.out.println(many.descriptorString());

        System.out.println(MethodTypeDesc.of(ConstantDescs.CD_void).descriptorString());
    }
}
