# addin-rabbitmq-stream

Внешняя компонента для `1С:Предприятие` клиент `RabbitMQ Streams`.

Компонента написана на `Rust` и основана на официальной библиотеке [rabbitmq-stream-rust-client](https://github.com/rabbitmq/rabbitmq-stream-rust-client).

Ссылки:
- https://www.rabbitmq.com/docs/streams - документация по стримам.
- https://rabbitmq.github.io/rabbitmq-stream-java-client/stable/htmlsingle/ - документация `Java` клиента, но т.к. для `Rust` клиента такой документации нет, то лучше читать эту.
- https://github.com/rabbitmq/rabbitmq-stream-rust-client/tree/main/examples - примеры на `Rust`, чтобы понять как можно реализовать ту или иную фукциональность.

## API

Реализованы не все возможности. Примеры кода смотри в тестах:
- [Обработка Тесты](conf/DataProcessors/Тесты/Forms/Форма/Ext/Form/Module.bsl)

В компоненте реализованы 2 объекта:
- `RabbitMQ.Stream.Producer`
- `RabbitMQ.Stream.Consumer`

Но каждый из них включает функциональность объекта `EnvironmentBuilder`, т.к. технология внешних компонент не позволяет передавать объекты.

### Общие свойства всех объектов 
- `LastError: Строка` - в случае исключения будет содержать текст ошибки.

### Объект `EnvironmentBuilder`
Методы:
- `SetHost(host: Строка)`
- `SetPort(port: Число)`
- `SetUsername(username: Строка)`
- `SetPassword(password: Строка)`
- `SetVirtualHost(host: Строка)`
- `SetHeartbeat(heartbeat: Число)`
- `SetLoadBalancerMode(mode: Булево)`
- `AddClientCertificatesKeys(certificate_path: Строка, private_key_path: Строка)`
- `AddRootCertificates(certificate_path: Строка)`

### Объект `RabbitMQ.Stream.Producer`
Методы:
- `SetName(name: Строка)` - имя продюсера, имеет смысл вызывать до метода `Build`.
- `Build(stream: Строка)` - создает продюсера, после этого можно отправлять сообщения.
- `SetApplicationProperty(key: Строка, value: Строка|Число|Булево|Дата|ДвоичныеДанные)` - устанавливает свойства для нового сообщения.
- `AddMessage(data: ДвоичныеДанные)` - добавляет сообщение во внутренний массив, сообщению также устанавливаются `ApplicationProperties`, установленные методом `SetApplicationProperty`.
- `BatchSend()` - отправляет все накопленные сообщения, в случае ошибки будет брошено исключение.
- `Statuses(): Строка` - возвращает статусы отправленных сообщений, имеет смысл смотреть в случае неуспешного выполнения метода `BatchSend`.

### Объект `RabbitMQ.Stream.Consumer`
Методы:
- `SetName(name: Строка)` - имя консьюмера, имеет смысл вызывать до метода `Build`.
- `Build(stream: Строка)` - создает консьюмера, после этого можно получать сообщения.
- `Recv(timeout: Число): Булево` - таймаут задается в миллсекундах, возвращает `Истина` - если сообщение получено, `Ложь` - если вышел таймаут.
- `MessageBody(): ДвоичныеДанные` - возвращает тело последнего сообщения.
- `ApplicationProperty(key: Строка): Строка|Число|Булево|Дата|ДвоичныеДанные|Неопределено` - возвращает значение свойства, либо `Неопределено`, если свойство отсутствует.
- `Offset(): ДвоичныеДанные` - возвращает смещение, которое представляет собой число `u64` но в виде `ДвоичныеДанные`, т.к. технология не позволяет передавать целыен числа больше `i32`.
- `StoreOffset(offset: ДвоичныеДанные)` - сохраняет оффсет, которые передается в формате числа `u64` записанное в `ДвоичныеДанные`, этот метод требуется вызывать, чтобы подтвердить получение сообщений.
