//Single object
//Use slices and references, not copies
//Facets held in a different structure.
//Use hashsets not hashmaps for subtraction.
//Cache n-2 sentences. get top word. only do the n-2 sentences. concurrent execution. if we compute the frequency map only once, we end up losing flexibility essential to the flashcard app. could probably precompute different internal states based on whether the user got the card right or wrong.

#[allow(unused_imports)]
use serde::{Serialize, Deserialize};
//use tokio;
#[allow(unused_imports)]
use std::{
    collections::{HashSet, HashMap},
    time::{Duration, Instant},
    sync::{Arc, Mutex},
    fs::{File},
    io::{Read},
};

//Gem: vec of strings, hashset of facets, hashset of strings
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Gem {
    //pub number: usize,
    pub sides: HashMap<usize, String>,
    pub unknown_facets: HashSet<String>,
}
//GemCollection: gems_by_size_index indexes borrowed mutable references to gems by the number of facets they have. gems_by_facet_index indexes borrowed mutable references to gems by the facet-strings they have (e.g "physics": vec of gems here). Lifetime references.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct GemCollection<'a> {
    pub gems: HashMap<usize, Gem>,
    pub known_facets: HashSet<String>,
    pub gems_by_size_index: HashMap<usize, HashSet<usize>>,
    pub gems_by_facet_index: HashMap<String, HashSet<usize>>,
    pub total_frequency_list: HashMap<String, usize>,
    pub unused_thing: &'a str,
}

impl<'a> GemCollection<'a> {
    pub async fn index_all_gems_by_number(&mut self) {
        for (number, gem) in self.gems.iter_mut() {
            if gem.unknown_facets.len() > 0 {
                self.gems_by_size_index
                .entry(
                    gem.unknown_facets.len()
                )
                .or_insert(HashSet::new())
                .insert(number.clone());
            }
            for facet in gem.unknown_facets.iter() {
                self.gems_by_facet_index
                    .entry(
                        facet.clone()
                    )
                    .or_insert(HashSet::new())
                    .insert(number.clone());
            }
        }
        self.total_frequency_list = self.create_frequency_hashmap_from_facets_of_n2_gem_indices(HashSet::from_iter(0..self.gems.len()));
        //println!("{:?}", self.gems_by_size_index);
    }
    //Okay, let's use serde to read in a list of gem structs represented in json in this format:
    //[{"sides":{"0":"In mechanical engineering, the Beale number is a parameter that characterizes the performance of Stirling engines"},"unknown_facets":["mechanical engineering", "Beale number", "Stirling engines"]}...]
    pub async fn read_gems_from_file(file_path: &str) -> Result<GemCollection<'a>, String> {
        let mut file = File::open(file_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        //Prints the first 300 chars 
        println!("{}", &contents[0..1000]);
        let gems: Vec<Gem> = serde_json::from_str(&contents).map_err(|e| format!("{}", e))?;
        //println!("{:?}", gems);
        let mut gem_collection = GemCollection {
            gems: HashMap::new(),
            known_facets: HashSet::new(),
            gems_by_size_index: HashMap::new(),
            gems_by_facet_index: HashMap::new(),
            total_frequency_list: HashMap::new(),
            unused_thing: "",
        };
        for (number, gem) in gems.iter().enumerate() {
            gem_collection.gems.insert(number, gem.clone());
        }
        Ok(gem_collection)
    }

    //Here, will use tokio spawn to run the indexing in parallel.
    pub async fn display_all_gems_in_order_of_difficulty(&'a mut self) {
        self.known_facets = HashSet::new();
        self.index_all_gems_by_number().await;

        for _ in 0..200 {
            let non_empty_keys = self.gems_by_size_index.keys().filter(|&key| self.gems_by_size_index.get(key).unwrap().len() > 0);
            //We get the minimum number from the keys of gems_by_size_index, and the second minimum number, filtering out any keys that point to empty hashsets
            let min_number = &non_empty_keys.clone()
                                            .min()
                                            .unwrap();
            let min_number_2 = &non_empty_keys.clone()
                                            .skip(1)
                                            .min()
                                            .unwrap();
            //We fetch all the Gem indices from gems_by_size_index for the minimum number, as HashSets:
            let gem_indices_for_n1: HashSet<usize> = HashSet::from_iter(
                self.gems_by_size_index
                    .get(&min_number)
                    .unwrap()
                    .clone()
            );
            let gem_indices_for_n2: HashSet<usize> = HashSet::from_iter(
                self.gems_by_size_index
                    .get(&min_number_2)
                    .unwrap()
                    .clone()
            );
            //We create a frequency hashmap by counting how many times each facet appears in total for all n_2 gems:
            let frequency_hashmap = self.create_frequency_hashmap_from_facets_of_n2_gem_indices(gem_indices_for_n2);
            //We get the facets with the highest frequency, sampling only from n_1 gems:
            let top_gem_facets: HashSet<String> = self.choose_max_n1_gem_facets_by_frequency_hashmap(gem_indices_for_n1, &frequency_hashmap, 2);
            println!("{:?}", top_gem_facets);
            //Most of the time, there's only one facet but sometimes there are up to 7 or 8. So what we want to do now is take the facet names, get the appropriate gem indices from gems_by_facet_index, and find the intersection of those gem indices with the gem indices for n_1, and n_2.
            //We get the indices of the gems that have the top n1 gem facets:
            let mut top_gem_indices: HashSet<usize> = HashSet::new();
            for facet in top_gem_facets.iter() {
                top_gem_indices = top_gem_indices.union(
                    self.gems_by_facet_index
                        .get(facet)
                        .as_ref()
                        .clone()
                        .unwrap()
                    ).cloned().collect();
            }
            //We could do this, but that would be borrowing "self" twice, so we need to edit the gem_collection in place:
            //self.gems_by_size_index.retain(|_, v| v.intersection(&top_gem_indices).count() > 0);
            //Now all we need to do is go through self.gems and subtract top_gem_facets from each gem's unknown_facet field, since now we know them. Before that, we remove the gem's number from gems_by_size_index, adding it to the gems_by_size_index "above" it (e.g if it's currently indexed under '3', we add it to '4').
            for gem_index in top_gem_indices.iter() {
                let mut gem = self.gems.get_mut(gem_index).unwrap();
                self.gems_by_size_index
                    .get_mut(&gem.unknown_facets.len())
                    .unwrap()
                    .remove(gem_index);
                //The index above might not exist, so we need to create it if it doesn't:
                self.gems_by_size_index
                    .entry(gem.unknown_facets.len() + 1)
                    .or_insert(HashSet::new())
                    .insert(*gem_index);
                gem.unknown_facets = gem.unknown_facets.difference(&top_gem_facets).cloned().collect();
            }
                //Now, we remove the top_gem_indices from each facet index in top_gem_facets via difference, the same as last time. We need to access
                for facet in top_gem_facets.iter() {
                //self.gems_by_facet_index.get_mut(facet).unwrap().difference(&top_gem_indices);
                //Quick sanity check, when we call difference, it returns a new HashSet, so we can't just replace it. And as we noted last time, there's no such thing as 'difference with'. So we need to do this:
                let facet_indices = self.gems_by_facet_index.get_mut(facet).unwrap();
                facet_indices.retain(|&gem_index| !top_gem_indices.contains(&gem_index));
            }
        }
    }

    fn create_frequency_hashmap_from_facets_of_n2_gem_indices(&self, gem_indices_for_n2: HashSet<usize>) -> HashMap<String, usize> {
        let mut frequency_hashmap: HashMap<String, usize> = HashMap::new();
        for gem_index in gem_indices_for_n2.iter() {
            let gem = self.gems.get(gem_index).unwrap();
            for facet in gem.unknown_facets.iter() {
                frequency_hashmap.entry(facet.clone())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
            }
        }
        frequency_hashmap
    }

    fn choose_max_n1_gem_facets_by_frequency_hashmap(&self, gem_indices_for_n1: HashSet<usize>, frequency_hashmap: &HashMap<String, usize>, _minimum_viable_hashmap_number: usize) -> HashSet<String> {
        //Here, we're essentially just going: ok, so I have all of these gem indices. And I have a map that tells me that so-and-so facet occurred 5 or 10 or however many times. Now I just need to look at each gem, and see how often each of its facets occurs in the map. Then I just average out that frequency, call it 'weight', and get the gem with the highest weight.
        let mut top_gem_facets: HashSet<String> = HashSet::new();
        //let mut top_gem_sides: HashMap<usize, String> = HashMap::new();
        let mut max_weight: f64 = 0.0;
        for gem_index in gem_indices_for_n1.iter() {
            let gem = self.gems.get(gem_index).unwrap();
            let mut weight: f64 = 0.0;
            for facet in gem.unknown_facets.iter() {
                //There's a possibility the facet might not be in the hashmap, so we need to check for that:
                if let Some(facet_weight) = frequency_hashmap.get(facet) {
                    weight += *facet_weight as f64;
                }
                //weight += *frequency_hashmap.get(facet).unwrap() as f64;
            }
            weight /= gem.unknown_facets.len() as f64;
            if weight > max_weight && gem.unknown_facets.len() > 0{
                top_gem_facets = gem.unknown_facets.clone();
                //top_gem_sides = gem.sides.clone();
                max_weight = weight;
            }
        }
        //println!("{:?}", top_gem_sides);
        if top_gem_facets.len() == 0 {
            //Then I can simply call myself again, but with self.total_frequency_list
            top_gem_facets = self.choose_max_n1_gem_facets_by_frequency_hashmap(gem_indices_for_n1, &self.total_frequency_list.clone(), _minimum_viable_hashmap_number);
        }
        top_gem_facets
    }
}


#[tokio::main]
async fn main() {
    let mut gem_collection = GemCollection::read_gems_from_file("src/gems.json").await.unwrap();
    let now = Instant::now();
    gem_collection.index_all_gems_by_number().await;
    let elapsed = now.elapsed();
    println!("Indexing all gems by number took {} microseconds", elapsed.as_micros());
    let now = Instant::now();
    gem_collection.display_all_gems_in_order_of_difficulty().await;
    let elapsed = now.elapsed();
    println!("Displaying all gems took {} microseconds", elapsed.as_micros());
}
