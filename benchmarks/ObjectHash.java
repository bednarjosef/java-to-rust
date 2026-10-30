// Objects of many classes hashed as `Object`s, as a large program's hash sets and maps hash
// their keys: a set of 300 objects probed for each of them (`ImmutableCollections.SetN.probe`
// is the Minecraft server's), where two classes in three hash by their address and the others
// override `hashCode`, the classes of each kind numbered apart. `hashCode` through `Object`
// finds the object's class among all of them.
public class ObjectHash {
    static final class K000 { final int v; K000(int v) { this.v = v; } }
    static final class K001 { final int v; K001(int v) { this.v = v; } }
    static final class K002 { final int v; K002(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K002 k && k.v == v; } public int hashCode() { return 2 * 31 + v; } }
    static final class K003 { final int v; K003(int v) { this.v = v; } }
    static final class K004 { final int v; K004(int v) { this.v = v; } }
    static final class K005 { final int v; K005(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K005 k && k.v == v; } public int hashCode() { return 5 * 31 + v; } }
    static final class K006 { final int v; K006(int v) { this.v = v; } }
    static final class K007 { final int v; K007(int v) { this.v = v; } }
    static final class K008 { final int v; K008(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K008 k && k.v == v; } public int hashCode() { return 8 * 31 + v; } }
    static final class K009 { final int v; K009(int v) { this.v = v; } }
    static final class K010 { final int v; K010(int v) { this.v = v; } }
    static final class K011 { final int v; K011(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K011 k && k.v == v; } public int hashCode() { return 11 * 31 + v; } }
    static final class K012 { final int v; K012(int v) { this.v = v; } }
    static final class K013 { final int v; K013(int v) { this.v = v; } }
    static final class K014 { final int v; K014(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K014 k && k.v == v; } public int hashCode() { return 14 * 31 + v; } }
    static final class K015 { final int v; K015(int v) { this.v = v; } }
    static final class K016 { final int v; K016(int v) { this.v = v; } }
    static final class K017 { final int v; K017(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K017 k && k.v == v; } public int hashCode() { return 17 * 31 + v; } }
    static final class K018 { final int v; K018(int v) { this.v = v; } }
    static final class K019 { final int v; K019(int v) { this.v = v; } }
    static final class K020 { final int v; K020(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K020 k && k.v == v; } public int hashCode() { return 20 * 31 + v; } }
    static final class K021 { final int v; K021(int v) { this.v = v; } }
    static final class K022 { final int v; K022(int v) { this.v = v; } }
    static final class K023 { final int v; K023(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K023 k && k.v == v; } public int hashCode() { return 23 * 31 + v; } }
    static final class K024 { final int v; K024(int v) { this.v = v; } }
    static final class K025 { final int v; K025(int v) { this.v = v; } }
    static final class K026 { final int v; K026(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K026 k && k.v == v; } public int hashCode() { return 26 * 31 + v; } }
    static final class K027 { final int v; K027(int v) { this.v = v; } }
    static final class K028 { final int v; K028(int v) { this.v = v; } }
    static final class K029 { final int v; K029(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K029 k && k.v == v; } public int hashCode() { return 29 * 31 + v; } }
    static final class K030 { final int v; K030(int v) { this.v = v; } }
    static final class K031 { final int v; K031(int v) { this.v = v; } }
    static final class K032 { final int v; K032(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K032 k && k.v == v; } public int hashCode() { return 32 * 31 + v; } }
    static final class K033 { final int v; K033(int v) { this.v = v; } }
    static final class K034 { final int v; K034(int v) { this.v = v; } }
    static final class K035 { final int v; K035(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K035 k && k.v == v; } public int hashCode() { return 35 * 31 + v; } }
    static final class K036 { final int v; K036(int v) { this.v = v; } }
    static final class K037 { final int v; K037(int v) { this.v = v; } }
    static final class K038 { final int v; K038(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K038 k && k.v == v; } public int hashCode() { return 38 * 31 + v; } }
    static final class K039 { final int v; K039(int v) { this.v = v; } }
    static final class K040 { final int v; K040(int v) { this.v = v; } }
    static final class K041 { final int v; K041(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K041 k && k.v == v; } public int hashCode() { return 41 * 31 + v; } }
    static final class K042 { final int v; K042(int v) { this.v = v; } }
    static final class K043 { final int v; K043(int v) { this.v = v; } }
    static final class K044 { final int v; K044(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K044 k && k.v == v; } public int hashCode() { return 44 * 31 + v; } }
    static final class K045 { final int v; K045(int v) { this.v = v; } }
    static final class K046 { final int v; K046(int v) { this.v = v; } }
    static final class K047 { final int v; K047(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K047 k && k.v == v; } public int hashCode() { return 47 * 31 + v; } }
    static final class K048 { final int v; K048(int v) { this.v = v; } }
    static final class K049 { final int v; K049(int v) { this.v = v; } }
    static final class K050 { final int v; K050(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K050 k && k.v == v; } public int hashCode() { return 50 * 31 + v; } }
    static final class K051 { final int v; K051(int v) { this.v = v; } }
    static final class K052 { final int v; K052(int v) { this.v = v; } }
    static final class K053 { final int v; K053(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K053 k && k.v == v; } public int hashCode() { return 53 * 31 + v; } }
    static final class K054 { final int v; K054(int v) { this.v = v; } }
    static final class K055 { final int v; K055(int v) { this.v = v; } }
    static final class K056 { final int v; K056(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K056 k && k.v == v; } public int hashCode() { return 56 * 31 + v; } }
    static final class K057 { final int v; K057(int v) { this.v = v; } }
    static final class K058 { final int v; K058(int v) { this.v = v; } }
    static final class K059 { final int v; K059(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K059 k && k.v == v; } public int hashCode() { return 59 * 31 + v; } }
    static final class K060 { final int v; K060(int v) { this.v = v; } }
    static final class K061 { final int v; K061(int v) { this.v = v; } }
    static final class K062 { final int v; K062(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K062 k && k.v == v; } public int hashCode() { return 62 * 31 + v; } }
    static final class K063 { final int v; K063(int v) { this.v = v; } }
    static final class K064 { final int v; K064(int v) { this.v = v; } }
    static final class K065 { final int v; K065(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K065 k && k.v == v; } public int hashCode() { return 65 * 31 + v; } }
    static final class K066 { final int v; K066(int v) { this.v = v; } }
    static final class K067 { final int v; K067(int v) { this.v = v; } }
    static final class K068 { final int v; K068(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K068 k && k.v == v; } public int hashCode() { return 68 * 31 + v; } }
    static final class K069 { final int v; K069(int v) { this.v = v; } }
    static final class K070 { final int v; K070(int v) { this.v = v; } }
    static final class K071 { final int v; K071(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K071 k && k.v == v; } public int hashCode() { return 71 * 31 + v; } }
    static final class K072 { final int v; K072(int v) { this.v = v; } }
    static final class K073 { final int v; K073(int v) { this.v = v; } }
    static final class K074 { final int v; K074(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K074 k && k.v == v; } public int hashCode() { return 74 * 31 + v; } }
    static final class K075 { final int v; K075(int v) { this.v = v; } }
    static final class K076 { final int v; K076(int v) { this.v = v; } }
    static final class K077 { final int v; K077(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K077 k && k.v == v; } public int hashCode() { return 77 * 31 + v; } }
    static final class K078 { final int v; K078(int v) { this.v = v; } }
    static final class K079 { final int v; K079(int v) { this.v = v; } }
    static final class K080 { final int v; K080(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K080 k && k.v == v; } public int hashCode() { return 80 * 31 + v; } }
    static final class K081 { final int v; K081(int v) { this.v = v; } }
    static final class K082 { final int v; K082(int v) { this.v = v; } }
    static final class K083 { final int v; K083(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K083 k && k.v == v; } public int hashCode() { return 83 * 31 + v; } }
    static final class K084 { final int v; K084(int v) { this.v = v; } }
    static final class K085 { final int v; K085(int v) { this.v = v; } }
    static final class K086 { final int v; K086(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K086 k && k.v == v; } public int hashCode() { return 86 * 31 + v; } }
    static final class K087 { final int v; K087(int v) { this.v = v; } }
    static final class K088 { final int v; K088(int v) { this.v = v; } }
    static final class K089 { final int v; K089(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K089 k && k.v == v; } public int hashCode() { return 89 * 31 + v; } }
    static final class K090 { final int v; K090(int v) { this.v = v; } }
    static final class K091 { final int v; K091(int v) { this.v = v; } }
    static final class K092 { final int v; K092(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K092 k && k.v == v; } public int hashCode() { return 92 * 31 + v; } }
    static final class K093 { final int v; K093(int v) { this.v = v; } }
    static final class K094 { final int v; K094(int v) { this.v = v; } }
    static final class K095 { final int v; K095(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K095 k && k.v == v; } public int hashCode() { return 95 * 31 + v; } }
    static final class K096 { final int v; K096(int v) { this.v = v; } }
    static final class K097 { final int v; K097(int v) { this.v = v; } }
    static final class K098 { final int v; K098(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K098 k && k.v == v; } public int hashCode() { return 98 * 31 + v; } }
    static final class K099 { final int v; K099(int v) { this.v = v; } }
    static final class K100 { final int v; K100(int v) { this.v = v; } }
    static final class K101 { final int v; K101(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K101 k && k.v == v; } public int hashCode() { return 101 * 31 + v; } }
    static final class K102 { final int v; K102(int v) { this.v = v; } }
    static final class K103 { final int v; K103(int v) { this.v = v; } }
    static final class K104 { final int v; K104(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K104 k && k.v == v; } public int hashCode() { return 104 * 31 + v; } }
    static final class K105 { final int v; K105(int v) { this.v = v; } }
    static final class K106 { final int v; K106(int v) { this.v = v; } }
    static final class K107 { final int v; K107(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K107 k && k.v == v; } public int hashCode() { return 107 * 31 + v; } }
    static final class K108 { final int v; K108(int v) { this.v = v; } }
    static final class K109 { final int v; K109(int v) { this.v = v; } }
    static final class K110 { final int v; K110(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K110 k && k.v == v; } public int hashCode() { return 110 * 31 + v; } }
    static final class K111 { final int v; K111(int v) { this.v = v; } }
    static final class K112 { final int v; K112(int v) { this.v = v; } }
    static final class K113 { final int v; K113(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K113 k && k.v == v; } public int hashCode() { return 113 * 31 + v; } }
    static final class K114 { final int v; K114(int v) { this.v = v; } }
    static final class K115 { final int v; K115(int v) { this.v = v; } }
    static final class K116 { final int v; K116(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K116 k && k.v == v; } public int hashCode() { return 116 * 31 + v; } }
    static final class K117 { final int v; K117(int v) { this.v = v; } }
    static final class K118 { final int v; K118(int v) { this.v = v; } }
    static final class K119 { final int v; K119(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K119 k && k.v == v; } public int hashCode() { return 119 * 31 + v; } }
    static final class K120 { final int v; K120(int v) { this.v = v; } }
    static final class K121 { final int v; K121(int v) { this.v = v; } }
    static final class K122 { final int v; K122(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K122 k && k.v == v; } public int hashCode() { return 122 * 31 + v; } }
    static final class K123 { final int v; K123(int v) { this.v = v; } }
    static final class K124 { final int v; K124(int v) { this.v = v; } }
    static final class K125 { final int v; K125(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K125 k && k.v == v; } public int hashCode() { return 125 * 31 + v; } }
    static final class K126 { final int v; K126(int v) { this.v = v; } }
    static final class K127 { final int v; K127(int v) { this.v = v; } }
    static final class K128 { final int v; K128(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K128 k && k.v == v; } public int hashCode() { return 128 * 31 + v; } }
    static final class K129 { final int v; K129(int v) { this.v = v; } }
    static final class K130 { final int v; K130(int v) { this.v = v; } }
    static final class K131 { final int v; K131(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K131 k && k.v == v; } public int hashCode() { return 131 * 31 + v; } }
    static final class K132 { final int v; K132(int v) { this.v = v; } }
    static final class K133 { final int v; K133(int v) { this.v = v; } }
    static final class K134 { final int v; K134(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K134 k && k.v == v; } public int hashCode() { return 134 * 31 + v; } }
    static final class K135 { final int v; K135(int v) { this.v = v; } }
    static final class K136 { final int v; K136(int v) { this.v = v; } }
    static final class K137 { final int v; K137(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K137 k && k.v == v; } public int hashCode() { return 137 * 31 + v; } }
    static final class K138 { final int v; K138(int v) { this.v = v; } }
    static final class K139 { final int v; K139(int v) { this.v = v; } }
    static final class K140 { final int v; K140(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K140 k && k.v == v; } public int hashCode() { return 140 * 31 + v; } }
    static final class K141 { final int v; K141(int v) { this.v = v; } }
    static final class K142 { final int v; K142(int v) { this.v = v; } }
    static final class K143 { final int v; K143(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K143 k && k.v == v; } public int hashCode() { return 143 * 31 + v; } }
    static final class K144 { final int v; K144(int v) { this.v = v; } }
    static final class K145 { final int v; K145(int v) { this.v = v; } }
    static final class K146 { final int v; K146(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K146 k && k.v == v; } public int hashCode() { return 146 * 31 + v; } }
    static final class K147 { final int v; K147(int v) { this.v = v; } }
    static final class K148 { final int v; K148(int v) { this.v = v; } }
    static final class K149 { final int v; K149(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K149 k && k.v == v; } public int hashCode() { return 149 * 31 + v; } }
    static final class K150 { final int v; K150(int v) { this.v = v; } }
    static final class K151 { final int v; K151(int v) { this.v = v; } }
    static final class K152 { final int v; K152(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K152 k && k.v == v; } public int hashCode() { return 152 * 31 + v; } }
    static final class K153 { final int v; K153(int v) { this.v = v; } }
    static final class K154 { final int v; K154(int v) { this.v = v; } }
    static final class K155 { final int v; K155(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K155 k && k.v == v; } public int hashCode() { return 155 * 31 + v; } }
    static final class K156 { final int v; K156(int v) { this.v = v; } }
    static final class K157 { final int v; K157(int v) { this.v = v; } }
    static final class K158 { final int v; K158(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K158 k && k.v == v; } public int hashCode() { return 158 * 31 + v; } }
    static final class K159 { final int v; K159(int v) { this.v = v; } }
    static final class K160 { final int v; K160(int v) { this.v = v; } }
    static final class K161 { final int v; K161(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K161 k && k.v == v; } public int hashCode() { return 161 * 31 + v; } }
    static final class K162 { final int v; K162(int v) { this.v = v; } }
    static final class K163 { final int v; K163(int v) { this.v = v; } }
    static final class K164 { final int v; K164(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K164 k && k.v == v; } public int hashCode() { return 164 * 31 + v; } }
    static final class K165 { final int v; K165(int v) { this.v = v; } }
    static final class K166 { final int v; K166(int v) { this.v = v; } }
    static final class K167 { final int v; K167(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K167 k && k.v == v; } public int hashCode() { return 167 * 31 + v; } }
    static final class K168 { final int v; K168(int v) { this.v = v; } }
    static final class K169 { final int v; K169(int v) { this.v = v; } }
    static final class K170 { final int v; K170(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K170 k && k.v == v; } public int hashCode() { return 170 * 31 + v; } }
    static final class K171 { final int v; K171(int v) { this.v = v; } }
    static final class K172 { final int v; K172(int v) { this.v = v; } }
    static final class K173 { final int v; K173(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K173 k && k.v == v; } public int hashCode() { return 173 * 31 + v; } }
    static final class K174 { final int v; K174(int v) { this.v = v; } }
    static final class K175 { final int v; K175(int v) { this.v = v; } }
    static final class K176 { final int v; K176(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K176 k && k.v == v; } public int hashCode() { return 176 * 31 + v; } }
    static final class K177 { final int v; K177(int v) { this.v = v; } }
    static final class K178 { final int v; K178(int v) { this.v = v; } }
    static final class K179 { final int v; K179(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K179 k && k.v == v; } public int hashCode() { return 179 * 31 + v; } }
    static final class K180 { final int v; K180(int v) { this.v = v; } }
    static final class K181 { final int v; K181(int v) { this.v = v; } }
    static final class K182 { final int v; K182(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K182 k && k.v == v; } public int hashCode() { return 182 * 31 + v; } }
    static final class K183 { final int v; K183(int v) { this.v = v; } }
    static final class K184 { final int v; K184(int v) { this.v = v; } }
    static final class K185 { final int v; K185(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K185 k && k.v == v; } public int hashCode() { return 185 * 31 + v; } }
    static final class K186 { final int v; K186(int v) { this.v = v; } }
    static final class K187 { final int v; K187(int v) { this.v = v; } }
    static final class K188 { final int v; K188(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K188 k && k.v == v; } public int hashCode() { return 188 * 31 + v; } }
    static final class K189 { final int v; K189(int v) { this.v = v; } }
    static final class K190 { final int v; K190(int v) { this.v = v; } }
    static final class K191 { final int v; K191(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K191 k && k.v == v; } public int hashCode() { return 191 * 31 + v; } }
    static final class K192 { final int v; K192(int v) { this.v = v; } }
    static final class K193 { final int v; K193(int v) { this.v = v; } }
    static final class K194 { final int v; K194(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K194 k && k.v == v; } public int hashCode() { return 194 * 31 + v; } }
    static final class K195 { final int v; K195(int v) { this.v = v; } }
    static final class K196 { final int v; K196(int v) { this.v = v; } }
    static final class K197 { final int v; K197(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K197 k && k.v == v; } public int hashCode() { return 197 * 31 + v; } }
    static final class K198 { final int v; K198(int v) { this.v = v; } }
    static final class K199 { final int v; K199(int v) { this.v = v; } }
    static final class K200 { final int v; K200(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K200 k && k.v == v; } public int hashCode() { return 200 * 31 + v; } }
    static final class K201 { final int v; K201(int v) { this.v = v; } }
    static final class K202 { final int v; K202(int v) { this.v = v; } }
    static final class K203 { final int v; K203(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K203 k && k.v == v; } public int hashCode() { return 203 * 31 + v; } }
    static final class K204 { final int v; K204(int v) { this.v = v; } }
    static final class K205 { final int v; K205(int v) { this.v = v; } }
    static final class K206 { final int v; K206(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K206 k && k.v == v; } public int hashCode() { return 206 * 31 + v; } }
    static final class K207 { final int v; K207(int v) { this.v = v; } }
    static final class K208 { final int v; K208(int v) { this.v = v; } }
    static final class K209 { final int v; K209(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K209 k && k.v == v; } public int hashCode() { return 209 * 31 + v; } }
    static final class K210 { final int v; K210(int v) { this.v = v; } }
    static final class K211 { final int v; K211(int v) { this.v = v; } }
    static final class K212 { final int v; K212(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K212 k && k.v == v; } public int hashCode() { return 212 * 31 + v; } }
    static final class K213 { final int v; K213(int v) { this.v = v; } }
    static final class K214 { final int v; K214(int v) { this.v = v; } }
    static final class K215 { final int v; K215(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K215 k && k.v == v; } public int hashCode() { return 215 * 31 + v; } }
    static final class K216 { final int v; K216(int v) { this.v = v; } }
    static final class K217 { final int v; K217(int v) { this.v = v; } }
    static final class K218 { final int v; K218(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K218 k && k.v == v; } public int hashCode() { return 218 * 31 + v; } }
    static final class K219 { final int v; K219(int v) { this.v = v; } }
    static final class K220 { final int v; K220(int v) { this.v = v; } }
    static final class K221 { final int v; K221(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K221 k && k.v == v; } public int hashCode() { return 221 * 31 + v; } }
    static final class K222 { final int v; K222(int v) { this.v = v; } }
    static final class K223 { final int v; K223(int v) { this.v = v; } }
    static final class K224 { final int v; K224(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K224 k && k.v == v; } public int hashCode() { return 224 * 31 + v; } }
    static final class K225 { final int v; K225(int v) { this.v = v; } }
    static final class K226 { final int v; K226(int v) { this.v = v; } }
    static final class K227 { final int v; K227(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K227 k && k.v == v; } public int hashCode() { return 227 * 31 + v; } }
    static final class K228 { final int v; K228(int v) { this.v = v; } }
    static final class K229 { final int v; K229(int v) { this.v = v; } }
    static final class K230 { final int v; K230(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K230 k && k.v == v; } public int hashCode() { return 230 * 31 + v; } }
    static final class K231 { final int v; K231(int v) { this.v = v; } }
    static final class K232 { final int v; K232(int v) { this.v = v; } }
    static final class K233 { final int v; K233(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K233 k && k.v == v; } public int hashCode() { return 233 * 31 + v; } }
    static final class K234 { final int v; K234(int v) { this.v = v; } }
    static final class K235 { final int v; K235(int v) { this.v = v; } }
    static final class K236 { final int v; K236(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K236 k && k.v == v; } public int hashCode() { return 236 * 31 + v; } }
    static final class K237 { final int v; K237(int v) { this.v = v; } }
    static final class K238 { final int v; K238(int v) { this.v = v; } }
    static final class K239 { final int v; K239(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K239 k && k.v == v; } public int hashCode() { return 239 * 31 + v; } }
    static final class K240 { final int v; K240(int v) { this.v = v; } }
    static final class K241 { final int v; K241(int v) { this.v = v; } }
    static final class K242 { final int v; K242(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K242 k && k.v == v; } public int hashCode() { return 242 * 31 + v; } }
    static final class K243 { final int v; K243(int v) { this.v = v; } }
    static final class K244 { final int v; K244(int v) { this.v = v; } }
    static final class K245 { final int v; K245(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K245 k && k.v == v; } public int hashCode() { return 245 * 31 + v; } }
    static final class K246 { final int v; K246(int v) { this.v = v; } }
    static final class K247 { final int v; K247(int v) { this.v = v; } }
    static final class K248 { final int v; K248(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K248 k && k.v == v; } public int hashCode() { return 248 * 31 + v; } }
    static final class K249 { final int v; K249(int v) { this.v = v; } }
    static final class K250 { final int v; K250(int v) { this.v = v; } }
    static final class K251 { final int v; K251(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K251 k && k.v == v; } public int hashCode() { return 251 * 31 + v; } }
    static final class K252 { final int v; K252(int v) { this.v = v; } }
    static final class K253 { final int v; K253(int v) { this.v = v; } }
    static final class K254 { final int v; K254(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K254 k && k.v == v; } public int hashCode() { return 254 * 31 + v; } }
    static final class K255 { final int v; K255(int v) { this.v = v; } }
    static final class K256 { final int v; K256(int v) { this.v = v; } }
    static final class K257 { final int v; K257(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K257 k && k.v == v; } public int hashCode() { return 257 * 31 + v; } }
    static final class K258 { final int v; K258(int v) { this.v = v; } }
    static final class K259 { final int v; K259(int v) { this.v = v; } }
    static final class K260 { final int v; K260(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K260 k && k.v == v; } public int hashCode() { return 260 * 31 + v; } }
    static final class K261 { final int v; K261(int v) { this.v = v; } }
    static final class K262 { final int v; K262(int v) { this.v = v; } }
    static final class K263 { final int v; K263(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K263 k && k.v == v; } public int hashCode() { return 263 * 31 + v; } }
    static final class K264 { final int v; K264(int v) { this.v = v; } }
    static final class K265 { final int v; K265(int v) { this.v = v; } }
    static final class K266 { final int v; K266(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K266 k && k.v == v; } public int hashCode() { return 266 * 31 + v; } }
    static final class K267 { final int v; K267(int v) { this.v = v; } }
    static final class K268 { final int v; K268(int v) { this.v = v; } }
    static final class K269 { final int v; K269(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K269 k && k.v == v; } public int hashCode() { return 269 * 31 + v; } }
    static final class K270 { final int v; K270(int v) { this.v = v; } }
    static final class K271 { final int v; K271(int v) { this.v = v; } }
    static final class K272 { final int v; K272(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K272 k && k.v == v; } public int hashCode() { return 272 * 31 + v; } }
    static final class K273 { final int v; K273(int v) { this.v = v; } }
    static final class K274 { final int v; K274(int v) { this.v = v; } }
    static final class K275 { final int v; K275(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K275 k && k.v == v; } public int hashCode() { return 275 * 31 + v; } }
    static final class K276 { final int v; K276(int v) { this.v = v; } }
    static final class K277 { final int v; K277(int v) { this.v = v; } }
    static final class K278 { final int v; K278(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K278 k && k.v == v; } public int hashCode() { return 278 * 31 + v; } }
    static final class K279 { final int v; K279(int v) { this.v = v; } }
    static final class K280 { final int v; K280(int v) { this.v = v; } }
    static final class K281 { final int v; K281(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K281 k && k.v == v; } public int hashCode() { return 281 * 31 + v; } }
    static final class K282 { final int v; K282(int v) { this.v = v; } }
    static final class K283 { final int v; K283(int v) { this.v = v; } }
    static final class K284 { final int v; K284(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K284 k && k.v == v; } public int hashCode() { return 284 * 31 + v; } }
    static final class K285 { final int v; K285(int v) { this.v = v; } }
    static final class K286 { final int v; K286(int v) { this.v = v; } }
    static final class K287 { final int v; K287(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K287 k && k.v == v; } public int hashCode() { return 287 * 31 + v; } }
    static final class K288 { final int v; K288(int v) { this.v = v; } }
    static final class K289 { final int v; K289(int v) { this.v = v; } }
    static final class K290 { final int v; K290(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K290 k && k.v == v; } public int hashCode() { return 290 * 31 + v; } }
    static final class K291 { final int v; K291(int v) { this.v = v; } }
    static final class K292 { final int v; K292(int v) { this.v = v; } }
    static final class K293 { final int v; K293(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K293 k && k.v == v; } public int hashCode() { return 293 * 31 + v; } }
    static final class K294 { final int v; K294(int v) { this.v = v; } }
    static final class K295 { final int v; K295(int v) { this.v = v; } }
    static final class K296 { final int v; K296(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K296 k && k.v == v; } public int hashCode() { return 296 * 31 + v; } }
    static final class K297 { final int v; K297(int v) { this.v = v; } }
    static final class K298 { final int v; K298(int v) { this.v = v; } }
    static final class K299 { final int v; K299(int v) { this.v = v; } public boolean equals(Object o) { return o instanceof K299 k && k.v == v; } public int hashCode() { return 299 * 31 + v; } }

    static Object[] table;

    // The slot of `key` in `table`, an open-addressing set as `SetN` is.
    static int probe(Object key) {
        int index = Math.floorMod(key.hashCode() ^ 0x55555555, table.length);
        while (true) {
            Object element = table[index];
            if (element == null) {
                return -index - 1;
            } else if (element.equals(key)) {
                return index;
            } else if (++index == table.length) {
                index = 0;
            }
        }
    }

    static long run(Object[] keys, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < keys.length; i++) {
                sum += probe(keys[i]);
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        int i = args.length;
        Object[] keys = { new K000(i), new K001(i), new K002(i), new K003(i), new K004(i), new K005(i), new K006(i), new K007(i), new K008(i), new K009(i), new K010(i), new K011(i), new K012(i), new K013(i), new K014(i), new K015(i), new K016(i), new K017(i), new K018(i), new K019(i), new K020(i), new K021(i), new K022(i), new K023(i), new K024(i), new K025(i), new K026(i), new K027(i), new K028(i), new K029(i), new K030(i), new K031(i), new K032(i), new K033(i), new K034(i), new K035(i), new K036(i), new K037(i), new K038(i), new K039(i), new K040(i), new K041(i), new K042(i), new K043(i), new K044(i), new K045(i), new K046(i), new K047(i), new K048(i), new K049(i), new K050(i), new K051(i), new K052(i), new K053(i), new K054(i), new K055(i), new K056(i), new K057(i), new K058(i), new K059(i), new K060(i), new K061(i), new K062(i), new K063(i), new K064(i), new K065(i), new K066(i), new K067(i), new K068(i), new K069(i), new K070(i), new K071(i), new K072(i), new K073(i), new K074(i), new K075(i), new K076(i), new K077(i), new K078(i), new K079(i), new K080(i), new K081(i), new K082(i), new K083(i), new K084(i), new K085(i), new K086(i), new K087(i), new K088(i), new K089(i), new K090(i), new K091(i), new K092(i), new K093(i), new K094(i), new K095(i), new K096(i), new K097(i), new K098(i), new K099(i), new K100(i), new K101(i), new K102(i), new K103(i), new K104(i), new K105(i), new K106(i), new K107(i), new K108(i), new K109(i), new K110(i), new K111(i), new K112(i), new K113(i), new K114(i), new K115(i), new K116(i), new K117(i), new K118(i), new K119(i), new K120(i), new K121(i), new K122(i), new K123(i), new K124(i), new K125(i), new K126(i), new K127(i), new K128(i), new K129(i), new K130(i), new K131(i), new K132(i), new K133(i), new K134(i), new K135(i), new K136(i), new K137(i), new K138(i), new K139(i), new K140(i), new K141(i), new K142(i), new K143(i), new K144(i), new K145(i), new K146(i), new K147(i), new K148(i), new K149(i), new K150(i), new K151(i), new K152(i), new K153(i), new K154(i), new K155(i), new K156(i), new K157(i), new K158(i), new K159(i), new K160(i), new K161(i), new K162(i), new K163(i), new K164(i), new K165(i), new K166(i), new K167(i), new K168(i), new K169(i), new K170(i), new K171(i), new K172(i), new K173(i), new K174(i), new K175(i), new K176(i), new K177(i), new K178(i), new K179(i), new K180(i), new K181(i), new K182(i), new K183(i), new K184(i), new K185(i), new K186(i), new K187(i), new K188(i), new K189(i), new K190(i), new K191(i), new K192(i), new K193(i), new K194(i), new K195(i), new K196(i), new K197(i), new K198(i), new K199(i), new K200(i), new K201(i), new K202(i), new K203(i), new K204(i), new K205(i), new K206(i), new K207(i), new K208(i), new K209(i), new K210(i), new K211(i), new K212(i), new K213(i), new K214(i), new K215(i), new K216(i), new K217(i), new K218(i), new K219(i), new K220(i), new K221(i), new K222(i), new K223(i), new K224(i), new K225(i), new K226(i), new K227(i), new K228(i), new K229(i), new K230(i), new K231(i), new K232(i), new K233(i), new K234(i), new K235(i), new K236(i), new K237(i), new K238(i), new K239(i), new K240(i), new K241(i), new K242(i), new K243(i), new K244(i), new K245(i), new K246(i), new K247(i), new K248(i), new K249(i), new K250(i), new K251(i), new K252(i), new K253(i), new K254(i), new K255(i), new K256(i), new K257(i), new K258(i), new K259(i), new K260(i), new K261(i), new K262(i), new K263(i), new K264(i), new K265(i), new K266(i), new K267(i), new K268(i), new K269(i), new K270(i), new K271(i), new K272(i), new K273(i), new K274(i), new K275(i), new K276(i), new K277(i), new K278(i), new K279(i), new K280(i), new K281(i), new K282(i), new K283(i), new K284(i), new K285(i), new K286(i), new K287(i), new K288(i), new K289(i), new K290(i), new K291(i), new K292(i), new K293(i), new K294(i), new K295(i), new K296(i), new K297(i), new K298(i), new K299(i) };
        table = new Object[keys.length * 2];
        for (Object key : keys) {
            table[-probe(key) - 1] = key;
        }
        long check = 0;
        for (int warm = 0; warm < 2; warm++) {
            check += run(keys, 2000);
        }
        long start = System.nanoTime();
        long result = run(keys, 40000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        // The slots depend on the addresses, which differ from run to run: only their count shows.
        System.out.println("object hash result " + (result != 0) + " " + (check != 0));
        System.err.println("kernel-ms " + elapsed);
    }
}
