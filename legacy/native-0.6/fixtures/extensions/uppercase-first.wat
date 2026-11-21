(module
  (memory (export "memory") 1)
  (func (export "forge_alloc") (param i32) (result i32)
    (i32.const 32))
  (func (export "forge_transform") (param $ptr i32) (param $len i32) (result i64)
    (local $first i32)
    (if (i32.gt_u (local.get $len) (i32.const 0))
      (then
        (local.set $first (i32.load8_u (local.get $ptr)))
        (if (i32.and
              (i32.ge_u (local.get $first) (i32.const 97))
              (i32.le_u (local.get $first) (i32.const 122)))
          (then
            (i32.store8
              (local.get $ptr)
              (i32.sub (local.get $first) (i32.const 32)))))))
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len)))))
