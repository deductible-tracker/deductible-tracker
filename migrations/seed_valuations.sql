SET DEFINE OFF

-- Seed Valuation Categories
INSERT INTO val_categories (id, name) VALUES ('cat_appliances', 'Appliances');
INSERT INTO val_categories (id, name) VALUES ('cat_childrens_clothing', 'Children''s Clothing');
INSERT INTO val_categories (id, name) VALUES ('cat_furniture', 'Furniture');
INSERT INTO val_categories (id, name) VALUES ('cat_household_goods', 'Household Goods');
INSERT INTO val_categories (id, name) VALUES ('cat_mens_clothing', 'Men''s Clothing');
INSERT INTO val_categories (id, name) VALUES ('cat_womens_clothing', 'Women''s Clothing');
INSERT INTO val_categories (id, name) VALUES ('cat_electronics', 'Electronics & Computers');
INSERT INTO val_categories (id, name) VALUES ('cat_miscellaneous', 'Miscellaneous');

-- Seed Valuation Items
-- Appliances
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_ac', 'cat_appliances', 'Air Conditioner', 21, 93);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_dryer', 'cat_appliances', 'Dryer', 47, 93);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_stove_elec', 'cat_appliances', 'Electric Stove', 78, 156);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_freezer', 'cat_appliances', 'Freezer', 25, 100);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_stove_gas', 'cat_appliances', 'Gas Stove', 52, 130);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_heater', 'cat_appliances', 'Heater', 8, 23);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_microwave', 'cat_appliances', 'Microwave', 10, 50);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_refrigerator', 'cat_appliances', 'Refrigerator (Working)', 78, 259);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_washer', 'cat_appliances', 'Washing Machine', 41, 156);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_coffeemaker', 'cat_appliances', 'Coffee Maker', 4, 16);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('app_iron', 'cat_appliances', 'Iron', 3, 10);

-- Children's Clothing
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_blouse', 'cat_childrens_clothing', 'Blouse', 2, 8);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_boots', 'cat_childrens_clothing', 'Boots', 3, 21);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_coat', 'cat_childrens_clothing', 'Coat', 5, 21);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_dress', 'cat_childrens_clothing', 'Dress', 2, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_jacket', 'cat_childrens_clothing', 'Jacket', 3, 26);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_jeans', 'cat_childrens_clothing', 'Jeans', 4, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_pants', 'cat_childrens_clothing', 'Pants', 3, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_shirt', 'cat_childrens_clothing', 'Shirt', 2, 10);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_shoes', 'cat_childrens_clothing', 'Shoes', 3, 10);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_snowsuit', 'cat_childrens_clothing', 'Snowsuit', 4, 20);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('child_sweater', 'cat_childrens_clothing', 'Sweater', 2, 10);

-- Furniture
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_bed_full', 'cat_furniture', 'Bed (full, queen, king)', 52, 176);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_bed_single', 'cat_furniture', 'Bed (single)', 36, 104);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_chair_uph', 'cat_furniture', 'Chair (upholstered)', 26, 104);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_chest', 'cat_furniture', 'Chest', 26, 99);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_china', 'cat_furniture', 'China Cabinet', 89, 311);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_coffee_table', 'cat_furniture', 'Coffee Table', 15, 100);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_desk', 'cat_furniture', 'Desk', 26, 145);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_dresser', 'cat_furniture', 'Dresser', 20, 104);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_end_table', 'cat_furniture', 'End Table', 10, 75);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_kitchen_set', 'cat_furniture', 'Kitchen Set', 35, 176);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('furn_sofa', 'cat_furniture', 'Sofa', 36, 395);

-- Household Goods
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_blanket', 'cat_household_goods', 'Blanket', 3, 14);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_curtains', 'cat_household_goods', 'Curtains', 2, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_lamp_floor', 'cat_household_goods', 'Lamp, Floor', 6, 52);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_lamp_table', 'cat_household_goods', 'Lamp, Table', 3, 20);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_pillow', 'cat_household_goods', 'Pillow', 2, 8);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_rug_area', 'cat_household_goods', 'Area Rug', 2, 93);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('house_sheets', 'cat_household_goods', 'Sheets', 2, 9);

-- Men''s Clothing
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_jacket', 'cat_mens_clothing', 'Jacket', 8, 45);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_suit', 'cat_mens_clothing', 'Suit (2pc)', 5, 96);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_shirt', 'cat_mens_clothing', 'Shirt', 3, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_pants', 'cat_mens_clothing', 'Pants', 4, 23);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_shoes', 'cat_mens_clothing', 'Shoes', 3, 30);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('men_sweater', 'cat_mens_clothing', 'Sweater', 3, 12);

-- Women''s Clothing
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_suit', 'cat_womens_clothing', 'Suit (2pc)', 10, 96);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_blouse', 'cat_womens_clothing', 'Blouse', 3, 12);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_dress', 'cat_womens_clothing', 'Dress', 4, 28);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_pants', 'cat_womens_clothing', 'Pants', 4, 23);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_shoes', 'cat_womens_clothing', 'Shoes', 2, 30);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('women_sweater', 'cat_womens_clothing', 'Sweater', 4, 13);

-- Electronics & Computers
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_desktop', 'cat_electronics', 'Desktop Computer', 20, 415);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_laptop', 'cat_electronics', 'Laptop', 25, 415);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_monitor', 'cat_electronics', 'Monitor', 5, 51);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_printer', 'cat_electronics', 'Printer', 1, 155);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_tablet', 'cat_electronics', 'Tablet', 25, 150);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('elec_tv', 'cat_electronics', 'TV (Color Working)', 78, 233);

-- Miscellaneous
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('misc_bicycle', 'cat_miscellaneous', 'Bicycle', 5, 83);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('misc_books_hard', 'cat_miscellaneous', 'Book (hardback)', 1, 3);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('misc_books_paper', 'cat_miscellaneous', 'Book (paperback)', 0.59, 2);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('misc_luggage', 'cat_miscellaneous', 'Luggage', 5, 16);
INSERT INTO val_items (id, category_id, name, suggested_min, suggested_max) VALUES ('misc_vacuum', 'cat_miscellaneous', 'Vacuum Cleaner', 5, 67);

COMMIT;
