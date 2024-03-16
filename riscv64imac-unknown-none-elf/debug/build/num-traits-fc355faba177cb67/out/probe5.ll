; ModuleID = 'probe5.49179d40707b0c62-cgu.0'
source_filename = "probe5.49179d40707b0c62-cgu.0"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "riscv64"

@str.0 = internal unnamed_addr constant [25 x i8] c"attempt to divide by zero"
@alloc_e6758488a51c40069ade2309416f0500 = private unnamed_addr constant <{ [6 x i8] }> <{ [6 x i8] c"<anon>" }>, align 1
@alloc_7a1147a36efe2c0cf231da5103dd8572 = private unnamed_addr constant <{ ptr, [16 x i8] }> <{ ptr @alloc_e6758488a51c40069ade2309416f0500, [16 x i8] c"\06\00\00\00\00\00\00\00\02\00\00\00\1F\00\00\00" }>, align 8

; probe5::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe55probe17h913a8077b7996665E() unnamed_addr #0 {
start:
  %0 = call i1 @llvm.expect.i1(i1 false, i1 false)
  br i1 %0, label %panic.i, label %"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17h8990f5626e77d743E.exit"

panic.i:                                          ; preds = %start
; call core::panicking::panic
  call void @_ZN4core9panicking5panic17h4fa8ed2fbbe0401eE(ptr align 1 @str.0, i64 25, ptr align 8 @alloc_7a1147a36efe2c0cf231da5103dd8572) #3
  unreachable

"_ZN4core3num21_$LT$impl$u20$u32$GT$10div_euclid17h8990f5626e77d743E.exit": ; preds = %start
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(none)
declare i1 @llvm.expect.i1(i1, i1) #1

; core::panicking::panic
; Function Attrs: cold noinline noreturn nounwind
declare dso_local void @_ZN4core9panicking5panic17h4fa8ed2fbbe0401eE(ptr align 1, i64, ptr align 8) unnamed_addr #2

attributes #0 = { nounwind "target-cpu"="generic-rv64" "target-features"="+m,+a,+c" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(none) }
attributes #2 = { cold noinline noreturn nounwind "target-cpu"="generic-rv64" "target-features"="+m,+a,+c" }
attributes #3 = { noreturn nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"Code Model", i32 3}
!1 = !{!"rustc version 1.78.0-nightly (c67326b06 2024-03-15)"}
