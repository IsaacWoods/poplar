; ModuleID = 'probe2.7d4ab6948bcf0dc8-cgu.0'
source_filename = "probe2.7d4ab6948bcf0dc8-cgu.0"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "riscv64"

; core::f64::<impl f64>::to_int_unchecked
; Function Attrs: inlinehint nounwind
define dso_local i32 @"_ZN4core3f6421_$LT$impl$u20$f64$GT$16to_int_unchecked17h23a8f4ad75151fb2E"(double %self) unnamed_addr #0 {
start:
; call <f64 as core::convert::num::FloatToInt<i32>>::to_int_unchecked
  %_0 = call i32 @"_ZN65_$LT$f64$u20$as$u20$core..convert..num..FloatToInt$LT$i32$GT$$GT$16to_int_unchecked17h02f129ec6e4957bcE"(double %self) #2
  ret i32 %_0
}

; <f64 as core::convert::num::FloatToInt<i32>>::to_int_unchecked
; Function Attrs: inlinehint nounwind
define internal i32 @"_ZN65_$LT$f64$u20$as$u20$core..convert..num..FloatToInt$LT$i32$GT$$GT$16to_int_unchecked17h02f129ec6e4957bcE"(double %self) unnamed_addr #0 {
start:
  %0 = alloca i32, align 4
  %1 = fptosi double %self to i32
  store i32 %1, ptr %0, align 4
  %_0 = load i32, ptr %0, align 4, !noundef !2
  ret i32 %_0
}

; probe2::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe25probe17hec2eba74f2db66a9E() unnamed_addr #1 {
start:
; call core::f64::<impl f64>::to_int_unchecked
  %_1 = call i32 @"_ZN4core3f6421_$LT$impl$u20$f64$GT$16to_int_unchecked17h23a8f4ad75151fb2E"(double 1.000000e+00) #2
  ret void
}

attributes #0 = { inlinehint nounwind "target-cpu"="generic-rv64" "target-features"="+m,+a,+c" }
attributes #1 = { nounwind "target-cpu"="generic-rv64" "target-features"="+m,+a,+c" }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"Code Model", i32 3}
!1 = !{!"rustc version 1.78.0-nightly (c67326b06 2024-03-15)"}
!2 = !{}
