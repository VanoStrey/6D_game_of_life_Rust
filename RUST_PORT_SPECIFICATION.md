# Rust Port Specification: 6D Game of Life

Данный документ представляет собой полную и строгую техническую спецификацию существующей реализации проекта **`6D_game_of_life`** на Java (JavaFX 23 / Maven) и определяет требования к архитектуре, математической модели, структурам данных и подсистемам будущей реализации на **Rust**.

Спецификация составлена на основе построчного аудита исходного Java-кода. Все утверждения предыдущих предварительных отчетов были перепроверены непосредственно по исходным файлам.

---

## 1. Original Java Architecture

Исходный проект расположен в директории `6D_game_of_life/` и содержит 4 Java-класса в пакете `org.example`:

```
6D_game_of_life/
├── pom.xml
├── README.md
└── src/main/java/org/example/
    ├── Main.java
    ├── LogicGameOfLive.java
    ├── CubeVisibilityHandler.java
    └── CameraTransform.java
```

### 1.1. Класс `Main` (`Main.java`)
* **Роль**: Точка входа JavaFX Application, графический интерфейс предварительной настройки параметров, запуск 3D-окна.
* **Поля**:
  - `private int size = 6` — размер ребра гиперкуба (по умолчанию 6).
  - `private int dimensions = 4` — число измерений (по умолчанию 4).
  - `private int delta3DgameBordes = 3` — расстояние в ячейках между 3D-блоками в макро-решетке (по умолчанию 3).
  - `private double percentMinNeighbors = 20` — нижний процентный порог выживания/рождения.
  - `private double percentMaxNeighbors = 45` — верхний процентный порог выживания/рождения.
* **Публичный API**:
  - `void start(Stage primaryStage)` — жизненный цикл JavaFX. Строит форму ввода `GridPane` (400x300) с 5 полями `TextField` и кнопкой `Button("Запустить игру")`.
  - `static void main(String[] args)` — вызов `Application.launch(args)`.
* **Внутренняя логика**:
  - При клике на кнопку выполняется парсинг: `Integer.parseInt(...)` и `Double.parseDouble(...)` без блоков `try-catch`.
  - Вызывается закрытый метод `startGame(primaryStage)`, создающий сцену размером 1920x1080 со включенным буфером глубины (`depthBuffer = true`).

### 1.2. Класс `LogicGameOfLive` (`LogicGameOfLive.java`)
* **Роль**: Хранение 6D-состояния автомата, генерация случайного состояния, расчет следующего поколения по окрестности Мура, построение 3D-проекции.
* **Поля**:
  - `private Random random = new Random()` — ГПСЧ для инициализации.
  - `private boolean[][][][][][] gameBoard` — текущее 6D-состояние.
  - `private boolean[][][][][][] gameBoardUpdate` — буфер для расчета следующего поколения.
  - `public int size` — размер ребра активных измерений.
  - `public int dimensions` — эффективное число измерений ($1 \le \text{dimensions} \le 6$).
  - `public int delta3DgameBordes` — зазор между гиперсрезами при 3D-проекции.
  - `private int[] sizeInDimensions = new int[6]` — фактический размер каждого из 6 измерений.
  - `private int[] deltaNeighbors = new int[6]` — диапазон проверки соседей для каждого измерения (3 для активных, 1 для неактивных).
* **Публичный API**:
  - `LogicGameOfLive(int size, int dimensions, int delta3DgameBordes)` — конструктор;
  - `void randomGameBoard()` — случайная инициализация всех ячеек через `random.nextBoolean()`;
  - `void updateGameBoard(double divisorMinNeighbors, double divisorMaxNeighbors)` — шаг симуляции клеточного автомата;
  - `boolean[][][] gameBoardVizual()` — формирование 3D-массива видимости кубов;
  - `int[] sizeGameBoardVizual()` — вычисление трехмерных габаритов 3D-решетки `[sizeZ, sizeY, sizeX]`.

### 1.3. Класс `CubeVisibilityHandler` (`CubeVisibilityHandler.java`)
* **Роль**: Управление объектами 3D-сцены JavaFX (`javafx.scene.shape.Box`).
* **Поля**:
  - `private Box[][][] cubes` — трехмерный массив визуальных нод JavaFX;
  - `private int sizeX, sizeY, sizeZ` — габариты трехмерной сцены;
  - `private final double cubeSize = 100` — размер стороны куба в единицах сцены JavaFX.
* **Публичный API**:
  - `CubeVisibilityHandler(int sizeX, int sizeY, int sizeZ)` — конструктор, аллоцирует массив и создает все кубы через `createCubes()`;
  - `Group initGroup()` — собирает все созданные кубы в единый `javafx.scene.Group`;
  - `void updateCubesVisibility(boolean[][][] visibilityArray)` — обновляет свойство `cube.setVisible(true/false)`.

### 1.4. Класс `CameraTransform` (`CameraTransform.java`)
* **Роль**: Свободная камера (полет в 3D) и диспетчеризация пользовательского ввода.
* **Поля**:
  - `public Camera camera = new PerspectiveCamera(true)`;
  - `private double speedMovement = 20.0`, `private double speedRotation = 0.05`;
  - `cameraDeltaX = 1000`, `cameraDeltaY = 5500`, `cameraDeltaZ = -2500`;
  - `cameraRotationX = 53` (Pitch), `cameraRotationY = 0` (Yaw);
  - `private Set<KeyCode> pressedKeys = new HashSet<>()`;
  - Ссылки на `CubeVisibilityHandler`, `LogicGameOfLive`, пороговые проценты `percentMinNeighbors`, `percentMaxNeighbors`.
* **Публичный API**:
  - `CameraTransform(...)` — инициализация камеры (`farClip = 50000.0`, начальная позиция и углы);
  - `void UpdateTransform(Scene scene)` — регистрация слушателей `KeyPressed`, `KeyReleased`, `MousePressed`, `MouseDragged`.

### 1.5. Порядок вызовов и жизненный цикл системы
1. `Main.main()` $\to$ JavaFX runtime $\to$ `Main.start(primaryStage)`.
2. Пользователь нажимает кнопку в окне настроек $\to$ считываются текстовые поля.
3. `startGame()` инстанциирует:
   - `LogicGameOfLive` $\to$ выделяются 6D-массивы $\to$ `randomGameBoard()`;
   - `sizeGameBoardVizual()` $\to$ `CubeVisibilityHandler(sizes[2], sizes[1], sizes[0])` $\to$ создаются тысячи объектов `Box`;
   - `updateCubesVisibility()` синхронизирует начальное состояние;
   - `CameraTransform` навешивает обработчики событий на `Scene`;
   - `primaryStage.setScene(...)` отображает 3D-мир.
4. Интерактивный цикл (Event-driven):
   - При нажатии `SPACE` вызывается `logicGameOfLive.updateGameBoard(...)`, затем `logicGameOfLive.gameBoardVizual()`, затем `cubeVisibilityHandler.updateCubesVisibility(...)`.
   - При нажатии `R` генерируется новое случайное поле и обновляется видимость.
   - Мышь и клавиши WASDQE смещают и вращают камеру в реальном времени.

---

## 2. Simulation Model

* **Мерность пространства ($D$)**:
  Конструктор принимает параметр `dimensions`. Если $1 \le \text{dimensions} \le 6$, то число измерений сохраняется. Если передано значение вне этого диапазона, код принудительно выставляет `dimensions = 6`.
* **Состояние клетки**:
  Двоичное: `true` (живая) / `false` (мертвая).
* **Синхронность обновления (Double Buffering)**:
  Симуляция является строго синхронной. Следующее состояние всех клеток вычисляется на основе текущего состояния `gameBoard` и записывается во временный массив `gameBoardUpdate`. Только после полного завершения обхода всего пространства состояние `gameBoardUpdate` копируется обратно в `gameBoard`.
* **Вероятностная инициализация**:
  Метод `randomGameBoard()` заполняет все ячейки вызовом `java.util.Random.nextBoolean()`, что соответствует равномерному распределению с вероятностью $p = 0.5$.

---

## 3. Grid Representation

### 3.1. Хранение в Java
В Java состояние хранится в объекте `boolean[][][][][][] gameBoard`. Это 6-уровневое дерево ссылок.

### 3.2. Маппинг измерений и индексов
Вектор размеров измерений `sizeInDimensions` и вектор дельт `deltaNeighbors` заполняются следующим образом:
```java
for (int i = 0; i < dimensions; i++) {
    sizeInDimensions[i] = size;
    deltaNeighbors[i] = 3;
}
for (int i = dimensions; i < 6; i++) {
    sizeInDimensions[i] = 1;
    deltaNeighbors[i] = 1;
}
```
Массив инициализируется как:
```java
gameBoard = new boolean[sizeInDimensions[5]]
                       [sizeInDimensions[4]]
                       [sizeInDimensions[3]]
                       [sizeInDimensions[2]]
                       [sizeInDimensions[1]]
                       [sizeInDimensions[0]];
```
Обозначим координаты обращения к массиву `gameBoard[a][b][c][d][e][f]`:
* `a` (индекс 5) $\in [0, \text{sizeInDimensions}[5] - 1]$
* `b` (индекс 4) $\in [0, \text{sizeInDimensions}[4] - 1]$
* `c` (индекс 3) $\in [0, \text{sizeInDimensions}[3] - 1]$
* `d` (индекс 2) $\in [0, \text{sizeInDimensions}[2] - 1]$
* `e` (индекс 1) $\in [0, \text{sizeInDimensions}[1] - 1]$
* `f` (индекс 0) $\in [0, \text{sizeInDimensions}[0] - 1]$

**Следствие**: Измерения активируются «снизу вверх» (от младшего индекса `f` к старшему `a`):
* При $D = 1$: активна только координата `f` (размер `size`), остальные `a,b,c,d,e` имеют размер 1 (индекс 0).
* При $D = 2$: активны координаты `f` и `e`.
* При $D = 3$: активны координаты `f`, `e`, `d`.
* При $D = 4$: активны координаты `f`, `e`, `d`, `c`.
* При $D = 5$: активны координаты `f`, `e`, `d`, `c`, `b`.
* При $D = 6$: активны все 6 координат `f, e, d, c, b, a`.

---

## 4. Neighbor Algorithm

В методе `updateCell` окрестность исследуется шестью вложенными циклами:
`a1` от 0 до `deltaNeighbors[5] - 1`
...
`f1` от 0 до `deltaNeighbors[0] - 1`.

Для активного измерения переменная цикла принимает значения $\{0, 1, 2\}$, а смещение рассчитывается как $\delta = \text{coord} - 1 + \text{idx}$.
Для неактивного измерения переменная цикла принимает только значение $0$, а координата жестко фиксируется в 0.

### 4.1. Анализ условия `formatCounters`
В коде метода `formatCounters`:
```java
if (a1 + b1 + c1 + d1 + e1 + f1 == 0) {
    return null;
}
```
**Строгий математический факт**:
1. Переменные `a1, b1, c1, d1, e1, f1` неотрицательны. Их сумма равна нулю тогда и только тогда, когда:
   $$a1 = 0, \quad b1 = 0, \quad c1 = 0, \quad d1 = 0, \quad e1 = 0, \quad f1 = 0$$
2. Для активных измерений значение индекса $0$ соответствует относительному смещению $\delta = 0 - 1 = -1$.
3. Следовательно, условие `sum == 0` исключает **единственную точку** во всей окрестности — крайний отрицательный угол с вектором смещения $(-1, -1, \dots, -1)$.
4. Центральная точка клетки $(0, 0, \dots, 0)$ соответствует индексам $a1=1, b1=1, \dots, f1=1$ (для активных измерений). Сумма индексов равна $D \ge 1 \ne 0$.
5. **Вывод**: **Центральная клетка НЕ отбрасывается `formatCounters` и учитывается алгоритмом как один из своих собственных соседей!**

### 4.2. Фактическое количество проверяемых позиций
Для клетки, находящейся строго во внутренней области решетки (вдали от границ):
* Теоретическое число точек в гиперкубической окрестности: $3^D$.
* Отбрасывается условием `sum == 0`: ровно 1 точка $(-1, \dots, -1)$.
* Итого проверяется позиций: **$3^D - 1$**.
* Из них:
  - Истинных внешних соседей клетки: $3^D - 2$;
  - Сама текущая клетка: 1.

| Мерность $D$ | Полная окрестность $3^D$ | Проверяется в Java ($3^D - 1$) | Истинных соседей ($3^D - 2$) | Сама клетка |
| :---: | :---: | :---: | :---: | :---: |
| **1D** | 3 | **2** | 1 (только $+1$; сосед $-1$ отброшен) | 1 |
| **2D** | 9 | **8** | 7 (сосед $(-1,-1)$ отброшен) | 1 |
| **3D** | 27 | **26** | 25 (сосед $(-1,-1,-1)$ отброшен) | 1 |
| **4D** | 81 | **80** | 79 | 1 |
| **5D** | 243 | **242** | 241 | 1 |
| **6D** | 729 | **728** | 727 | 1 |

> [!IMPORTANT]
> Предыдущее предположение о классической окрестности Мура в 728 соседей опровергнуто: Java-код проверяет 727 соседей и саму клетку, теряя углового соседа $(-1, -1, -1, -1, -1, -1)$.

---

## 5. Transition Rules

### 5.1. Точный алгоритм в коде
```java
countAllNeighbours = countDieNeighbors + countLiveNeighbors;
int minNeighbors = (int) (countAllNeighbours / (100.0 / divisorMinNeighbors));
int maxNeighbors = (int) (countAllNeighbours / (100.0 / divisorMaxNeighbors));

return !(countLiveNeighbors > maxNeighbors || countLiveNeighbors < minNeighbors);
```

### 5.2. Деконструкция математических выражений
1. **Эквивалентность диапазона**:
   Условие `!(countLiveNeighbors > maxNeighbors || countLiveNeighbors < minNeighbors)` по законам алгебры логики эквивалентно:
   $$\text{minNeighbors} \le \text{countLiveNeighbors} \le \text{maxNeighbors}$$
2. **Расчет порогов**:
   Выражение `countAllNeighbours / (100.0 / divisor)` математически тождественно $\text{countAllNeighbours} \times \frac{\text{divisor}}{100.0}$.
   Приведение к `(int)` в Java выполняет **усечение к нулю (truncation towards zero)**, что для неотрицательных чисел соответствует математической функции $\lfloor \cdot \rfloor$ (`floor`).
3. **База вычисления порогов**:
   Пороги вычисляются относительно `countAllNeighbours` — числа **фактически проверенных и попавших в границы сетки ячеек** для данной конкретной клетки, а **не** относительно теоретического максимума.
4. **Скрытое влияние текущего состояния клетки**:
   Так как клетка проверяет сама себя:
   - Если клетка в текущем поколении **была жива**, она добавляет $+1$ в `countLiveNeighbors`.
   - Если клетка в текущем поколении **была мертва**, она добавляет $+1$ в `countDieNeighbors`.
   - При этом `countAllNeighbours` одинаков в обоих случаях.
   - Таким образом, для сохранения жизни живой клетке требуется на 1 живого соседа меньше, чем мертвой клетке для зарождения жизни!

---

## 6. Boundary Conditions

* **Тип границ**: Незамкнутые жесткие границы (No wrapping / Hard zero Dirichlet-like bounds). Тороидального заворачивания нет.
* **Поведение на границах**:
  Если координаты проверяемой точки выходят за пределы $[0, \text{sizeInDimensions}[i] - 1]$, проверка отсекается:
  ```java
  if (counters[5] >= 0 && counters[5] < sizeInDimensions[5] && ...)
  ```
  Вышедшие за границу координаты **не увеличивают** ни `countLiveNeighbors`, ни `countDieNeighbors`.
* **Динамическое сжатие окрестности**:
  Так как `countAllNeighbours` уменьшается на гранях, ребрах и вершинах, пороги `minNeighbors` и `maxNeighbors` пропорционально снижаются.

### 6.1. Примеры расчета окрестности в различных позициях

#### Пример 1: 3D-пространство ($\text{size} = 6, D = 3$, пороги 20% и 45%)
* **Центральная клетка $(2, 2, 2)$**:
  - Внутрь границ попадают все точки, кроме отброшенной $(-1, -1, -1)$.
  - `countAllNeighbours = 26`.
  - $\text{minNeighbors} = \lfloor 26 \times 0.20 \rfloor = \lfloor 5.2 \rfloor = 5$.
  - $\text{maxNeighbors} = \lfloor 26 \times 0.45 \rfloor = \lfloor 11.7 \rfloor = 11$.
  - Клетка жива в следующем шаге $\iff 5 \le \text{countLiveNeighbors} \le 11$.
* **Угол при нулевых координатах $(0, 0, 0)$**:
  - Для каждого активного измерения допустимы только смещения $\delta \in \{0, 1\}$.
  - Всего таких комбинаций: $2^3 = 8$.
  - Смещение $(-1, -1, -1)$ здесь даже не генерируется в пределах границ, поэтому условие `sum == 0` не отсекает валидных ячеек.
  - `countAllNeighbours = 8`.
  - $\text{minNeighbors} = \lfloor 8 \times 0.20 \rfloor = \lfloor 1.6 \rfloor = 1$.
  - $\text{maxNeighbors} = \lfloor 8 \times 0.45 \rfloor = \lfloor 3.6 \rfloor = 3$.
  - Клетка жива $\iff 1 \le \text{countLiveNeighbors} \le 3$.
* **Противоположный угол $(\text{size}-1, \text{size}-1, \text{size}-1) = (5, 5, 5)$**:
  - Допустимы смещения $\delta \in \{-1, 0\}$.
  - Комбинация $(-1, -1, -1)$ попадает внутрь решетки ($5-1=4 \ge 0$), **но исключается условием `sum == 0`!**
  - Поэтому здесь `countAllNeighbours = 2^3 - 1 = 7`.
  - $\text{minNeighbors} = \lfloor 7 \times 0.20 \rfloor = \lfloor 1.4 \rfloor = 1$.
  - $\text{maxNeighbors} = \lfloor 7 \times 0.45 \rfloor = \lfloor 3.15 \rfloor = 3$.
  - Клетка жива $\iff 1 \le \text{countLiveNeighbors} \le 3$.

#### Пример 2: 6D-пространство ($\text{size} = 6, D = 6$, пороги 20% и 45%)
* **В центре**: `countAllNeighbours = 728`.
  - $\text{minNeighbors} = \lfloor 728 \times 0.20 \rfloor = \lfloor 145.6 \rfloor = 145$.
  - $\text{maxNeighbors} = \lfloor 728 \times 0.45 \rfloor = \lfloor 327.6 \rfloor = 327$.
* **В углу $(0,0,0,0,0,0)$**: `countAllNeighbours = 2^6 = 64`.
  - $\text{minNeighbors} = \lfloor 64 \times 0.20 \rfloor = 12$.
  - $\text{maxNeighbors} = \lfloor 64 \times 0.45 \rfloor = 28$.
* **В углу $(5,5,5,5,5,5)$**: `countAllNeighbours = 2^6 - 1 = 63`.
  - $\text{minNeighbors} = \lfloor 63 \times 0.20 \rfloor = 12$.
  - $\text{maxNeighbors} = \lfloor 63 \times 0.45 \rfloor = 28$.

#### Пример 3: Вырожденный случай $\text{size} = 1$ (любой $D$)
* Существует ровно одна ячейка $(0, \dots, 0)$.
* Единственное допустимое смещение $\delta = (0, \dots, 0)$, для которого $a1=1, \dots, f1=1 \implies \text{sum} = D \ne 0$.
* `countAllNeighbours = 1`.
* $\text{minNeighbors} = \lfloor 1 \times 0.20 \rfloor = 0$.
* $\text{maxNeighbors} = \lfloor 1 \times 0.45 \rfloor = 0$.
* Если клетка мертва: $\text{countLiveNeighbors} = 0 \in [0, 0] \implies$ рождается!
* Если клетка жива: $\text{countLiveNeighbors} = 1 \notin [0, 0] \implies$ умирает!
* Ячейка строго осциллирует с периодом 2: $0 \to 1 \to 0 \to 1 \dots$

---

## 7. Coordinate System

### 7.1. 6D координаты клетки
Вектор координат: $(a, b, c, d, e, f)$, где $a, b, c, d, e, f \ge 0$.
* $f$ — измерение 0
* $e$ — измерение 1
* $d$ — измерение 2
* $c$ — измерение 3
* $b$ — измерение 4
* $a$ — измерение 5

### 7.2. 3D координаты в графической сцене
Связь между индексами Java-массива и осями координат сцены JavaFX:
В `CubeVisibilityHandler.java`:
```java
cube.setTranslateX(x * cubeSize);
cube.setTranslateY(y * cubeSize);
cube.setTranslateZ(z * cubeSize);
```
В `Main.java`:
```java
int[] sizes = logicGameOfLive.sizeGameBoardVizual();
CubeVisibilityHandler cubeVisibilityHandler = new CubeVisibilityHandler(sizes[2], sizes[1], sizes[0]);
```
Параметр `sizeX` конструктора получает `sizes[2]`.
Параметр `sizeY` конструктора получает `sizes[1]`.
Параметр `sizeZ` конструктора получает `sizes[0]`.

В `gameBoardVizual()`:
```java
gameBoardVizual[d + (c * (size + delta3DgameBordes))]
               [e + (b * (size + delta3DgameBordes))]
               [f + (a * (size + delta3DgameBordes))] = gameBoard[a][b][c][d][e][f];
```

**Фактическое сопоставление осей**:
$$\mathbf{X}_{3D} = d + c \cdot (\text{size} + \Delta)$$
$$\mathbf{Y}_{3D} = e + b \cdot (\text{size} + \Delta)$$
$$\mathbf{Z}_{3D} = f + a \cdot (\text{size} + \Delta)$$
где $\Delta = \text{delta3DgameBordes}$.

* Ось **$X$** составлена из измерений **2** ($d$, внутри блока) и **3** ($c$, позиция блока по $X$).
* Ось **$Y$** составлена из измерений **1** ($e$, внутри блока) и **4** ($b$, позиция блока по $Y$).
* Ось **$Z$** составлена из измерений **0** ($f$, внутри блока) и **5** ($a$, позиция блока по $Z$).

> [!WARNING]
> Первоначальный краткий отчет ошибочно поменял местами оси $X$ и $Z$. В фактическом Java-коде $X$ определяется координатами $d$ и $c$, а $Z$ — координатами $f$ и $a$.

---

## 8. 3D Projection

Многомерный гиперкуб проецируется в трехмерную решетку срезов (макро-сетку кубов).

Габариты 3D-массива определяются методом `sizeGameBoardVizual()`:
```java
switch (dimensions) {
    case 6: sizes[2] = (size + delta) * size; sizes[1] = (size + delta) * size; sizes[0] = (size + delta) * size; break;
    case 5: sizes[2] = (size + delta) * size; sizes[1] = (size + delta) * size; sizes[0] = size; break;
    case 4: sizes[2] = (size + delta) * size; sizes[1] = size;                  sizes[0] = size; break;
    case 3: sizes[2] = size;                  sizes[1] = size;                  sizes[0] = size; break;
    case 2: sizes[2] = 1;                     sizes[1] = size;                  sizes[0] = size; break;
    case 1: sizes[2] = 1;                     sizes[1] = 1;                     sizes[0] = size; break;
}
```

### Геометрическая интерпретация:
* **1D**: цепочка длины `size` вдоль оси $Z$ ($1 \times 1 \times \text{size}$).
* **2D**: плоский слой в плоскости $YZ$ ($1 \times \text{size} \times \text{size}$).
* **3D**: одиночный куб $\text{size} \times \text{size} \times \text{size}$.
* **4D**: одномерная линия из `size` трехмерных кубов вдоль оси $X$, разделенных зазором $\Delta$.
* **5D**: двумерная сетка $\text{size} \times \text{size}$ из 3D-кубов в плоскости $XY$.
* **6D**: трехмерная матрица $\text{size} \times \text{size} \times \text{size}$ из 3D-кубов в пространстве $XYZ$.

**Особенность распределения памяти в Java**:
Размерность массива берется как $(\text{size} + \Delta) \cdot \text{size}$. При этом максимальный занятый индекс равен $(\text{size} - 1) + (\text{size} - 1) \cdot (\text{size} + \Delta)$. На конце каждого измерения образуется «пустой зазор» длиной $\Delta$, для которого в JavaFX тем не менее выделяются невидимые кубы `Box`.

---

## 9. Rendering Model

* **Технология**: JavaFX 3D Scene Graph.
* **Используемые примитивы**: `javafx.scene.shape.Box` с ребром 100 единиц.
* **Материал**: `PhongMaterial(Color.DARKBLUE)`.
* **Управление отображением**:
  - При запуске создаются `sizes[2] * sizes[1] * sizes[0]` отдельных объектов `Box`.
  - На каждом шаге вызывается `cube.setVisible(visibilityArray[x][y][z])`.
  - Кубы никогда не удаляются и не создаются заново во время шагов симуляции.
* **Критическое ограничение производительности**:
  В JavaFX 3D отсутствуют инстансинг и объединение мешей. Для каждого куба выполняется независимый проход и хранение в памяти Java heap.
  Для $D=6, \text{size}=6, \Delta=3$:
  Общее число кубов в сцене:
  $$54 \times 54 \times 54 = 157\,464 \text{ объектов Box!}$$
  Это приводит к потреблению сотен мегабайт памяти и падению FPS до неиграбельных значений (< 1-2 FPS).

---

## 10. Camera and Input

### 10.1. Параметры камеры
* Класс: `javafx.scene.PerspectiveCamera(true)` (True 3D с фиксированным глазом).
* `farClip = 50000.0`.
* Начальное положение: $X = 1000.0, \quad Y = 5500.0, \quad Z = -2500.0$.
* Начальная ориентация: $\text{rotX} = 53.0^\circ$ (Pitch), $\text{rotY} = 0.0^\circ$ (Yaw).

### 10.2. Перемещение (Клавиатура)
Шаг перемещения: `speedMovement = 20.0`.
Движение осуществляется строго в **мировых координатах (World Space Translation)**, ориентация камеры на направление смещения **не влияет**:
* `W`: $Y \mathrel{-}= 20.0$ (вверх в координатах JavaFX)
* `S`: $Y \mathrel{+}= 20.0$ (вниз в координатах JavaFX)
* `A`: $X \mathrel{-}= 20.0$ (влево)
* `D`: $X \mathrel{+}= 20.0$ (вправо)
* `Q`: $Z \mathrel{-}= 20.0$ (назад / от сцены)
* `E`: $Z \mathrel{+}= 20.0$ (вперед / к сцене)

### 10.3. Вращение (Мышь)
* Событие: `MouseDragged` при удержании кнопки мыши.
* Чувствительность: `speedRotation = 0.05`.
* Формула изменения углов:
  $$\text{cameraRotationX} \mathrel{-}= (Y_{mouse} - Y_{initial}) \cdot 0.05$$
  $$\text{cameraRotationY} \mathrel{+}= (X_{mouse} - X_{initial}) \cdot 0.05$$
* Ограничений по углу тангажа (pitch clamping) нет — камера допускает полный переворот (flip).

### 10.4. Горячие клавиши
* `SPACE` — расчет следующего поколения и обновление видимости.
* `R` — рандомизация клеток и обновление видимости.
* `T` — сброс автомата в $D = 1$ с сохранением `size` и `delta3DgameBordes` (в Java приводит к гарантированному вылету с `ArrayIndexOutOfBoundsException`, если исходная размерность была $> 1$).

---

## 11. UI

### 11.1. Окно настроек
* Размер окна: 400x300, заголовок `"Настройки игры"`.
* Таблица элементов управления (`GridPane`, hgap=10, vgap=10):
  1. `"Размерность : "` $\to$ Текстовое поле, по умолчанию `"6"`.
  2. `"Мерность (от 1 до 6 включительно): "` $\to$ Текстовое поле, по умолчанию `"4"`.
  3. `"Расстояние между блоками : \n *когда измерений больше 3"` $\to$ Текстовое поле, по умолчанию `"3"`.
  4. `"Мин. соседи (%): "` $\to$ Текстовое поле, по умолчанию `"20.0"`.
  5. `"Макс. соседи (%): "` $\to$ Текстовое поле, по умолчанию `"45.0"`.
  6. Кнопка `"Запустить игру"`.

### 11.2. Окно игры
* Размер окна: 1920x1080, заголовок `"Игра"`.

### 11.3. Поведение при некорректном вводе и точки отказа Java
* Пустая строка, текст вместо чисел, пробелы $\to$ `NumberFormatException` в UI-потоке, игра не запускается.
* Отрицательный `size` или 0 $\to$ `NegativeArraySizeException` при создании массива.
* Значение `dimensions` $< 1$ или $> 6$ $\to$ конструктор молча устанавливает `dimensions = 6`.

---

## 12. Performance Analysis

### 12.1. Количественные показатели алгоритма

| Конфигурация ($D, \text{size}, \Delta$) | Всего клеток ($N = \text{size}^D$) | Проверок соседей на 1 клетку ($K$) | Всего итераций на 1 поколение ($N \times K$) | Аллокаций `int[6]` за 1 шаг симуляции | Объем мусора GC за 1 шаг | Число `Box` в сцене |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **3D, size=6, $\Delta=3$** | 216 | 26 | $5\,616$ | $5\,616$ | $\approx 224$ КБ | 216 |
| **4D, size=6, $\Delta=3$** (default) | $1\,296$ | 80 | $103\,680$ | $103\,680$ | $\approx 4.1$ МБ | $1\,944$ |
| **5D, size=6, $\Delta=3$** | $7\,776$ | 242 | $1\,881\,792$ | $1\,881\,792$ | $\approx 75$ МБ | $17\,496$ |
| **6D, size=6, $\Delta=3$** | $46\,656$ | 728 | $\mathbf{33\,965\,568}$ | $\mathbf{33\,965\,568}$ | $\mathbf{\approx 1.35}$ **ГБ** | $\mathbf{157\,464}$ |

### 12.2. Анализ причин низкой производительности Java
1. **Катастрофическая нагрузка на Garbage Collector**:
   Метод `formatCounters` создает `new int[6]` на каждой итерации проверки соседа. При $D=6$ один шаг генерирует **~34 миллиона временных массивов** (~1.35 ГБ аллокаций), приводя к «заиканию» симуляции из-за Stop-The-World пауз GC.
2. **Фрагментация памяти**:
   6-мерный массив указателей `boolean[][][][][][]` разбросан по куче, что приводит к 100% cache-misses процессора при обходе соседей.
3. **Отсутствие GPU-инстансинга**:
   Создание сотен тысяч отдельных `Box` перегружает JavaFX Scene Graph и OpenGL драйвер.

---

## 13. Rust Architecture

Предлагается чистая модульная архитектура с разделением вычислительного ядра и подсистем представления.

```
6d_game_of_life/
├── Cargo.toml
└── src/
    ├── main.rs                   # Bootstrap & Application loop
    ├── core/
    │   ├── mod.rs
    │   ├── coords.rs             # N-мерные координаты, плоские индексы, stride
    │   ├── grid.rs               # Плотный буфер состояния клеток (BitGrid / ByteGrid)
    │   ├── rules.rs              # Правила переходов (Legacy Java-совместимый и Strict Conway)
    │   └── simulation.rs         # Ядро симуляции, double-buffering, параллельный step()
    ├── projection/
    │   ├── mod.rs
    │   └── visual_mapping.rs     # ND -> 3D проецирование и генерация InstanceData
    ├── render/
    │   ├── mod.rs
    │   ├── pipeline.rs           # wgpu RenderPipeline (Instanced Cube Mesh)
    │   ├── instance.rs           # GPU Instance Buffer (положение, видимость)
    │   └── camera.rs             # 3D Perspective Camera, матрица вида и проекции
    └── ui/
        ├── mod.rs
        └── egui_overlay.rs       # Настройки симуляции, статистика FPS/Cells, управление
```

### 13.1. Спецификация модулей

1. **`core::coords`**:
   - *Ответственность*: Расчет смещений (strides) для $D \in [1, 6]$, преобразование кортежа $(a,b,c,d,e,f) \leftrightarrow \text{index}$, предрасчет относительных оффсетов окрестности.
   - *API*: `Coords6D`, `GridDimensions`, `NeighborOffsets`.
2. **`core::grid`**:
   - *Ответственность*: Непрерывный буфер памяти ячеек.
   - *API*: `get(idx) -> bool`, `set(idx, val)`, `len() -> usize`. Никаких аллокаций в горячем цикле.
3. **`core::rules`**:
   - *Ответственность*: Определение следующего состояния клетки на основе живых и всех соседей.
   - *API*: `RuleConfig { min_percent: f64, max_percent: f64, mode: RuleMode }`.
4. **`core::simulation`**:
   - *Ответственность*: Владение буферами `current` и `next`, выполнение шага симуляции.
   - *API*: `step()`, `randomize(seed, prob)`, `reset(dims)`.
5. **`projection::visual_mapping`**:
   - *Ответственность*: Преобразование $D$-мерной решетки в список видимых 3D-инстансов.
   - *API*: `generate_instances(&Grid) -> Vec<InstanceRaw>`.
6. **`render`**:
   - *Ответственность*: GPU-рендеринг кубов с инстансингом через единый Draw Call.
7. **`ui`**:
   - *Ответственность*: Панель параметров на `egui`, плавная смена конфигурации без перезапуска приложения.

### 13.2. Сравнение графических backend для Rust

| Критерий | `wgpu + egui` (Рекомендуется) | `Bevy` | `Macroquad` |
| :--- | :--- | :--- | :--- |
| **Instancing сотен тысяч кубов** | Идеально (чистый Draw Call инстансинга) | Отлично, но требует работы через ECS | Ограничено, требует низкоуровневых хаков |
| **Контроль над памятью и буферами** | Полный низкоуровневый контроль | Опосредованный (через Render World) | Минимальный |
| **Интеграция UI** | Нативная через `egui-wgpu` | Через Bevy UI (сложнее) | Встроенный UI / macroquad-egui |
| **Оверхед и время сборки** | Минимальный, быстрый build | Большой фреймворк, долгая компиляция | Очень быстрый |
| **Вердикт для данного проекта** | **Наилучший выбор**: надежность, скорость, GPU instancing | Избыточен по архитектуре | Слишком примитивен для больших 3D-сцен |

---

## 14. Storage Strategy

Сравнение структур данных для хранения гиперрешетки:

| Структура | Память ($6^6$ клеток) | Скорость чтения соседа | Cache Locality | Сложность реализации |
| :--- | :--- | :--- | :--- | :--- |
| `Vec<bool>` | 46.6 КБ | Быстро, но `bool` в Rust равен 1 байту | Хорошая | Минимальная |
| `Vec<u8>` (ByteGrid) | 46.6 КБ | **Максимальная** (чтение без битовых сдвигов `*ptr`) | Отличная (полностью в L2 кэше) | Простая |
| `bitvec / BitVec<u64>` | **5.8 КБ** | Требует битовых масок и сдвигов | **Идеальная** (целиком в L1 кэше процессора!) | Средняя |
| Разреженная хэш-таблица (`HashSet`) | МБ | Медленно (хэширование) | Плохая | Не подходит |

**Рекомендованная стратегия**:
* Для симуляции: **`Vec<u8>`** (где $1 = \text{alive}, 0 = \text{dead}$).
  *Причина*: В симуляции критически важна скорость произвольного доступа к соседям. При размере сетки $6^6$ весь буфер занимает всего 46.6 КБ, что свободно помещается даже в L2-кэш современного CPU. Доступ к `u8` не требует битовых масок и сдвигов (bit-twiddling), что обеспечивает максимальный IPC (instructions per cycle).
* Для предрасчета соседей: одномерный массив относительных оффсетов `[isize; 728]`, позволяющий вычислять адрес соседа простым сложением указателя без многомерных циклов.

---

## 15. Parallelization Strategy

1. **Модель параллелизма**:
   Поскольку клеточный автомат синхронен, новое значение ячейки $idx$ зависит исключительно от значений буфера `current` на шаге $t$. Любые две клетки нового поколения могут вычисляться независимо. Состояние Data Race исключено по построению.
2. **Использование `rayon`**:
   Буфер `next` разбивается на непересекающиеся слайсы (чанки) с помощью `par_chunks_mut()`:
   ```rust
   next_grid.par_chunks_mut(CHUNK_SIZE).enumerate().for_each(|(chunk_idx, chunk)| {
       // вычисление состояния клеток чанка
   });
   ```
3. **Double Buffering (Zero Allocation)**:
   В структуре симуляции хранятся два вектора:
   ```rust
   pub struct Simulation {
       current: Vec<u8>,
       next: Vec<u8>,
   }
   ```
   В конце шага выполняется `std::mem::swap(&mut self.current, &mut self.next)`. Никакого перевыделения памяти не происходит.

---

## 16. Testing Strategy

Для строгого доказательства эквивалентности поведения Rust-порта и Java-оригинала:

1. **Генератор верификационных оракулов (Golden Tests)**:
   - Экспорт тестовых состояний из Java: написать тестовый сценарий на Java, сохраняющий в JSON/бинарный файл состояние сетки на шагах $t=0, 1, 2, 5$ для фиксированных seed и размеров ($D=1, 2, 3, 4, 6$).
   - В Rust: интеграционный тест загружает `state_t0.bin`, выполняет шаги симуляции и побайтово сверяет `state_tN` с дампом Java.
2. **Модульные тесты математических инвариантов**:
   - `test_sum_zero_corner_exclusion`: проверка, что относительный угол $(-1, \dots, -1)$ отбрасывается в legacy-режиме.
   - `test_self_cell_inclusion`: проверка, что живая клетка дает $+1$ в `countLiveNeighbors`.
   - `test_dynamic_threshold_calculation`: проверка формулы `countAllNeighbours * divisor / 100.0` с усечением до целого.
   - `test_boundary_conditions`: сравнение `countAllNeighbours` на углах $(0, \dots, 0)$ и $(\text{size}-1, \dots, \text{size}-1)$.
   - `test_size_1_oscillation`: проверка осцилляции единственной клетки при $\text{size} = 1$.
3. **Тесты проекции**:
   - Проверка формул $X = d + c(\text{size}+\Delta)$, $Y = e + b(\text{size}+\Delta)$, $Z = f + a(\text{size}+\Delta)$ на соответствие осям JavaFX.

---

## 17. Java $\to$ Rust Mapping

| Java-компонент | Rust-компонент | Назначение | Поведение должно сохраниться? |
| :--- | :--- | :--- | :--- |
| `LogicGameOfLive` | `core::Simulation` | Управление шагом и состоянием | **Да** (математически идентично) |
| `boolean[][][][][][]` | `core::Grid` (`Vec<u8>`) | Хранение ячеек | **Да** (по логике данных; внутренняя структура оптимизирована) |
| `updateCell(...)` | `Simulation::compute_cell(...)` | Подсчет соседей и правила перехода | **Да** (в режиме Java-compatibility) |
| `formatCounters(...)` | Таблица смещений `NeighborOffsets` | Определение координат соседей | **Да** (с сохранением бага отсечения в Legacy-режиме) |
| `gameBoardVizual()` | `projection::visual_mapping` | Построение 3D-проекции | **Да** (расположение блоков и кубов строго сохраняется) |
| `CubeVisibilityHandler` | `render::InstanceBuffer` | Отображение кубов на экране | **Да** (визуальный результат идентичен; под капотом GPU instancing) |
| `CameraTransform` | `render::Camera` | Положение и управление камерой | **Да** (WASDQE, мышь, чувствительность, стартовая позиция) |
| `Main` (JavaFX GUI) | `ui::EguiApp` | Окно настроек и оверлей | Улучшено (добавлена валидация, слайдеры, отсутствие вылетов) |
| `Random.nextBoolean()` | `rand::Rng::gen_bool(0.5)` | Случайная инициализация | Да (равномерное распределение) |

---

## 18. Known Bugs in Java

При аудите исходного кода Java обнаружены следующие подтвержденные дефекты:

1. **Дефект окрестности в `formatCounters`**:
   Условие `a1 + b1 + c1 + d1 + e1 + f1 == 0` исключает угол $(-1, -1, -1, -1, -1, -1)$, но оставляет центральную клетку $(1, 1, 1, 1, 1, 1)$, включая ее в число соседей.
2. **Асимметрия решетки на границах**:
   Из-за дефекта (1) угол сетки с максимальными координатами имеет на 1 соседа меньше, чем угол с нулевыми координатами.
3. **Краш при нажатии клавиши `T`**:
   `logicGameOfLive` пересоздается с $D=1$, а `CubeVisibilityHandler` сохраняет старые размеры. При попытке обновления происходит `ArrayIndexOutOfBoundsException`.
4. **Вылеты парсинга UI (`NumberFormatException`)**:
   Поля ввода в `Main.java` не имеют валидации и защиты от нечислового ввода.
5. **Фиктивный `mainClass` в `pom.xml`**:
   Указан `<mainClass>your.main.ClassName</mainClass>`, из-за чего стандартный запуск `mvn javafx:run` завершается ошибкой конфигурации.
6. **Выделение неиспользуемых 3D-кубов**:
   В `sizeGameBoardVizual()` размер берется с запасом на концевой зазор, из-за чего JavaFX создает тысячи бесполезных невидимых `Box`.
7. **Перепутанные подписи осей**:
   Поле `sizeField` подписано в UI как "Размерность", хотя задает размер стороны куба `size`.

---

## 19. Intentional Behavioral Changes

В будущей Rust-реализации планируются следующие осознанные улучшения:

1. **Режимы правил симуляции (Rule Modes)**:
   - `Mode::JavaBugCompatible` (по умолчанию для тестов): 100% воспроизведение оригинального поведения Java со всеми граничными артефактами и самопроверкой клетки.
   - `Mode::StrictMoore`: каноническая окрестность Мура (ровно $3^D - 1$ истинных соседей без включения самой клетки и без потери угла $(-1, \dots, -1)$).
2. **Защита от сбоев UI**:
   Замена текстовых полей без валидации на удобные числовые слайдеры `egui` с гарантированно корректными диапазонами ($D \in [1, 6]$, $\text{size} \in [1, 64]$, $\Delta \in [0, 20]$).
3. **Безопасная смена размерности на лету**:
   Клавиша `T` (или UI-селектор) будет корректно пересоздавать и симуляцию, и буфер инстансинга без падений.
4. **Оптимизация зазоров**:
   В GPU Instance Buffer будут заноситься только фактически существующие ячейки без пустых концевых зазоров.

---

## 20. Open Questions

Перед началом написания Rust-кода необходимо зафиксировать решения по следующим архитектурным развилкам:

1. **Выбор графического стека**:
   - Вариант A (**Рекомендуемый**): `wgpu` + `winit` + `egui`. Максимальная производительность, GPU-инстансинг кубов, компактный бинарник.
   - Вариант B: `Bevy`. Быстрый прототип камеры и UI, но большой оверхед и зависимость от ECS.
2. **Поведение камеры**:
   - Вариант A: Сохранить специфическое поведение Java (абсолютное перемещение в World Space без учета направления взгляда камеры).
   - Вариант B: Реализовать классическую свободную камеру (Free-fly/FPS camera, где W/S двигают камеру вперед/назад вдоль вектора взгляда).
3. **Поддержка непрерывной автосимуляции**:
   В Java шаги выполняются только вручную по нажатию `SPACE`. Следует ли в Rust сразу добавить режим автоматического таймера (Play / Pause / Speed Slider)?
