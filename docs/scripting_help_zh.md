# PlotRedox — 脚本参考

脚本引擎使用 **Rhai**（一种 Rust 嵌入式脚本语言）。
其语法类似 Rust / JavaScript。本指南涵盖基础知识。

---

## 1. 变量

```rhai
let x = 42;            // 整数 (i64)
let y = 3.14;          // 浮点数 (f64)
let name = "hello";    // 字符串
let flag = true;       // 布尔值
```

- 使用 `let` 声明变量（必需，不同于 Python）。
- 变量默认是**可变的**。
- 使用 `const` 声明常量：`const PI_2 = PI() / 2.0;`

---

## 2. 打印与字符串插值

```rhai
print("hello world");
print(`x = ${x}, y = ${y}`);   // 反引号字符串支持插值
```

---

## 3. 运算符

| 运算符 | 含义 |
|----------|---------|
| `+  -  *  /` | 算术运算 |
| `%` | 取模 |
| `==  !=  <  >  <=  >=` | 比较运算 |
| `&&  \|\|  !` | 逻辑与、或、非 |
| `+=  -=  *=  /=` | 复合赋值 |

> **注意**：整数除以整数 = 整数（截断）。使用 `.0` 进行浮点除法：`7.0 / 2.0`

---

## 4. 控制流

```rhai
if x > 0 {
    print("positive");
} else if x == 0 {
    print("zero");
} else {
    print("negative");
}

// for 循环遍历数组
for item in [1, 2, 3] {
    print(item);
}

// while 循环
let i = 0;
while i < 10 {
    i += 1;
}
```

在循环中使用 `continue` 和 `break`。

---

## 5. 数组与映射

```rhai
let arr = [1, 2, 3, 4, 5];
arr.push(6);
let length = arr.len();

let map = #{};            // 空映射（对象）
map.name = "test";
map.value = 42;
// 或：let map = #{ name: "test", value: 42 };

let keys = map.keys();   // ["name", "value"]
```

---

## 6. 数据访问

您的数字化数据以全局 `data` 映射的形式提供：

```rhai
// data 是一个映射：分组名 -> 数据点数组
// 每个点：#{ x, y, px, py }
//   x, y  = 校准后的（逻辑）坐标
//   px, py = 图像上的像素坐标

for name in data.keys() {
    let pts = data[name];
    print(`${name}: ${pts.len()} 个点`);

    if pts.len() > 0 {
        print(`  第一个点: (${pts[0].x}, ${pts[0].y})`);
    }
}
```

使用 `col()` 从映射数组中提取列：
```rhai
let xs = col(data["Group 1"], "x");  // -> x 值数组
let ys = col(data["Group 1"], "y");
```

---

## 7. 可用函数

### 数学函数（标量）

| 函数 | 描述 | 示例 |
|----------|-------------|---------|
| `abs(x)` | 绝对值 | `abs(-3.0)` -> `3.0` |
| `sqrt(x)` | 平方根 | `sqrt(9.0)` -> `3.0` |
| `ln(x)` | 自然对数 | `ln(exp(1.0))` -> `1.0` |
| `log10(x)` | 以 10 为底的对数 | `log10(100.0)` -> `2.0` |
| `log2(x)` | 以 2 为底的对数 | `log2(8.0)` -> `3.0` |
| `exp(x)` | e^x | `exp(0.0)` -> `1.0` |
| `pow(x, y)` | x^y | `pow(2.0, 3)` -> `8.0` |
| `pow10(x)` | 10^x | `pow10(2.0)` -> `100.0` |
| `sin(x)` `cos(x)` `tan(x)` | 三角函数 | 弧度制 |
| `asin(x)` `acos(x)` `atan(x)` | 反三角函数 | 返回弧度 |
| `atan2(y, x)` | 双参数反正切 | |
| `floor(x)` `ceil(x)` `round(x)` | 取整 | |
| `round_to(x, y)`| 四舍五入到指定位数 | `round_to(2.3333, 2)` -> `2.33` |
| `PI()` | 圆周率常数 | `3.14159…` |

### 数组聚合

| 函数 | 描述 | 示例 |
|----------|-------------|---------|
| `sum(arr)` | 求和 | `sum([1,2,3])` -> `6` |
| `mean(arr)` | 算术均值 | `mean([2,4])` -> `3` |
| `min_val(arr)` | 最小值 | `min_val([3,1,2])` -> `1` |
| `max_val(arr)` | 最大值 | `max_val([3,1,2])` -> `3` |
| `std_dev(arr)` | 标准差 | （样本，n-1） |
| `variance(arr)` | 方差 | （样本，n-1） |

### 数组操作

| 函数 | 描述 | 示例 |
|----------|-------------|---------|
| `log10_array(arr)` | 按元素取 log₁₀ | `log10_array([10, 100])` -> `[1, 2]` |

### 数据辅助函数

| 函数 | 描述 | 示例 |
|----------|-------------|---------|
| `col(arr, "field")` | 从映射数组中提取字段 | `col(pts, "x")` |
| `extract_number(s)` | 提取字符串中的首个数字 | `extract_number("20kHz")` -> `20.0` |

### 回归与拟合

| 函数 | 返回值 | 描述 |
|----------|---------|-------------|
| `linreg(xs, ys)` | `#{ slope, intercept, r_squared }` | 线性回归 |
| `polyfit(xs, ys, deg)` | `#{ coeffs: [...], r_squared }` | 多项式拟合（1-N 阶） |
| `lstsq(A, b)` | `[c0, c1, ...]` | 最小二乘解 A·c = b（A 为二维数组） |

#### 示例 — 线性回归
```rhai
let xs = col(data["Group 1"], "x");
let ys = col(data["Group 1"], "y");
let fit = linreg(xs, ys);
print(`斜率 = ${fit.slope}, R² = ${fit.r_squared}`);
```

#### 示例 — 多项式拟合
```rhai
let fit = polyfit(xs, ys, 3);  // 三次多项式
for i in 0..fit.coeffs.len() {
    print(`  a${i} = ${fit.coeffs[i]}`);
}
```

#### 示例 — 最小二乘法（多变量）
```rhai
// 求解：y = c0 + c1*x1 + c2*x2
let A = [];
let b = [];
// ... 用 [1.0, x1_i, x2_i] 行填充 A，用 y_i 填充 b ...
let coeffs = lstsq(A, b);
```

---

## 8. 提示

- 整数运算会截断：`7 / 2` = `3`。使用 `7.0 / 2.0` 进行浮点除法。
- 字符串插值需要使用**反引号**：`` `value = ${x}` ``
- `data` 映射是只读的；脚本无法修改数字化的数据点。
- `print()` 的所有输出显示在右侧的输出面板中。
- 脚本创建的变量显示在左侧的工作区面板中 — 点击可查看详情。
