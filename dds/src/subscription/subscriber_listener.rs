use crate::subscription::subscriber::Subscriber;
use crate::infrastructure::listener::NoListener;

pub trait SubscriberListener {
    fn on_data_on_readers(&self, _the_subscriber: Subscriber);
}

impl SubscriberListener for NoListener {
    fn on_data_on_readers(&self, _the_subscriber: Subscriber) {
        todo!()
    }
}