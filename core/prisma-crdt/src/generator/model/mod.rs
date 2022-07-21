mod actions;
mod create;
mod create_params;
mod delete;
mod set_param;
mod sync_id;
mod update;

use crate::generator::prelude::*;


pub fn generate<'a>(model: &'a Model<'a>, datamodel: &Datamodel) -> TokenStream {
    if matches!(&model.typ, ModelType::Local {..}) {
        return quote!();
    }
    
	let name_snake = snake_ident(&model.name);

	let set_param_enums = set_param::definition(model, datamodel);
	let sync_id_struct = sync_id::definition(model, datamodel);
	let create_params = create_params::definition(model, datamodel);

	let create_struct = create::generate(model, datamodel);
	let update_struct = update::generate(model);
	let delete_struct = delete::generate(model);

	let actions_struct = actions::generate(model);

	quote!(
		pub mod #name_snake {
			#set_param_enums

			#sync_id_struct

			#create_params

			#create_struct

			// #update_struct

			// #delete_struct

			#actions_struct
		}
	)
}
