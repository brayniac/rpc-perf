use super::*;

use ::momento::cache::{CollectionTtl, SortedSetPutElementRequest, SortedSetPutElementsRequest};

pub async fn sorted_set_add(
    client: &mut CacheClient,
    config: &Config,
    cache_name: &str,
    request: workload::client::SortedSetAdd,
) -> std::result::Result<(), ResponseError> {
    if request.members.is_empty() {
        return Ok(());
    }

    SORTED_SET_ADD.increment();

    if request.members.len() == 1 {
        let (member, score) = request.members.first().unwrap();

        // here we are able to borrow from the Arc<[u8]> and get a &[u8]
        // we deref the &f64 to get a f64. It's Copy, so that's fine. Members
        // might be long slices, so avoiding a clone is very nice.

        let r = SortedSetPutElementRequest::new(cache_name, &*request.key, &**member, *score)
            .ttl(CollectionTtl::new(request.ttl, false));

        let result = timeout(
            config.client().unwrap().request_timeout(),
            client.send_request(r),
        )
        .await;

        record_result!(result, SORTED_SET_ADD)
    } else {
        // there's a couple of ways we can do this, but essentially we need to
        // transform our Arc<[u8]> into something that implements Into<Vec<u8>>
        // our obvious options are to just convert to_vec() to get a Vec<u8>,
        // but this requires copying the members.

        // this works, we use IntoIter because we can. It doesn't really make
        // much of a difference
        
        // let d: Vec<(Vec<u8>, f64)> = request
        //     .members
        //     .into_iter()
        //     .map(|(m, s)| (m.to_vec(), s))
        //     .collect();

        // the other way would be like how we do above, we can borrow from the
        // Arc<[u8]> to get a &[u8]. Since we're just going to borrow, we must
        // use plain iter() to get borrows from the Arc<T> that's still in the
        // `SortedSetAdd` request.

        // this does not work!
        let d: Vec<(&[u8], f64)> = request
            .members
            .iter()
            .map(|(m, s)| (&**m, *s))
            .collect();

        let r = SortedSetPutElementsRequest::new(cache_name, &*request.key, d)
            .ttl(CollectionTtl::new(request.ttl, false));

        let result = timeout(
            config.client().unwrap().request_timeout(),
            client.send_request(r),
        )
        .await;

        record_result!(result, SORTED_SET_ADD)
    }
}
