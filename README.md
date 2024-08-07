# 项目介绍
期现套利：
期现套利是指当同币种的期货和现货存在较大价差时，通过买入低价方、卖出高价方，当两者的价差缩小时进行平仓，即可收获价差缩小部分利润

市场采用 binance 现货杠杆和币本位合约
* 现货杠杆下单
* 币本位交割合约对冲

**采用纯Rust代码编写，用来学习练手**  
如果你使用Python，可以看看这里->https://github.com/lijiachang/okx_spot_future_arbitrage

# 项目进展
1. api实现
   - [x] 杠杆现货api & websocket
   - [x] 币本位合约api & websocket
2. spider相关
   - [ ] 交割合约的现货交易对信息(用于哪些交易对需要订阅盘口，以及交易的精度等信息)
   - [x] 盘口数据订阅
   - [x] 计算【现货-交割合约】的收益率排名
3. 策略相关
   - [ ] 数据库表设计（SQL）
     - [ ] 可选：优化为ORM实现
   - [ ] 基于 策略思路 设计文档实现策略代码

# 账号准备工作：
* binance账号改为统一账户模式

# 策略思路
1. 策略模块中通过ws实时获取当前仓位和资金余额
2. 获取数据处理模块获取实时收益率排名
3. 开仓逻辑：
   1. 按序处理收益率排名中的交易对
   2. 判断数据是否超时（大于制定时间，比如10s)
      1. 超时则跳过
   3. 检查当前是否允许开仓
      1. 策略是否开启
      2. 现货USDT是否充足（balance > per_order_usd)
      3. 仓位是否达到上限
      4. 收益率是否达标
   4. 两腿下单
      1. 简单逻辑均为超价限价单，比如spot，以asks[5]为买价，future 以 bids[5] 为卖价
      2. 现货和合约的价格精度，根据市场数据来处理
      3. 现货size精度需要根据市场数据来处理
4. 平仓逻辑
   1. 遍历当前持仓，获取对应的收益率，收益率低于平仓阈值的进行平仓处理
   2. 平仓操作亦是两腿下单，同时操作。
