;; Test module: step never returns (the master watchdog must restart it)
(module
  (func (export "abi_version") (result i32) i32.const 2)
  (func (export "step") (param i32) (loop $l (br $l))))
