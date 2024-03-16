; ModuleID = 'probe4.562a1b59fbc250c4-cgu.0'
source_filename = "probe4.562a1b59fbc250c4-cgu.0"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "riscv64"

; probe4::probe
; Function Attrs: nounwind
define dso_local void @_ZN6probe45probe17hefc5e5f732ee4d8eE() unnamed_addr #0 {
start:
  %0 = alloca i32, align 4
  store i32 1, ptr %0, align 4
  %_0.i = load i32, ptr %0, align 4, !noundef !2
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.cttz.i32(i32, i1 immarg) #1

attributes #0 = { nounwind "target-cpu"="generic-rv64" "target-features"="+m,+a,+c" }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"Code Model", i32 3}
!1 = !{!"rustc version 1.78.0-nightly (c67326b06 2024-03-15)"}
!2 = !{}
