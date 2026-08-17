use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub trait StringProcessor: Send + Sync {
    fn process<'a>(&'a self, s: &'a str) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;
}

pub async fn walk_json<P: StringProcessor + ?Sized>(value: &mut Value, processor: &P) {
    match value {
        Value::String(s) => {
            *s = processor.process(s).await;
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                Box::pin(walk_json(item, processor)).await;
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj.iter_mut() {
                Box::pin(walk_json(val, processor)).await;
            }
        }
        _ => {}
    }
}
